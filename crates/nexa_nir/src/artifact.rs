use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{NirType, UnverifiedModule, VerificationError, VerifiedModule, Verifier};

/// Magic string for private compiler NIR artifacts.
pub const NIR_ARTIFACT_MAGIC: &str = "FUTAO-NIR";

/// Current private NIR schema version.
pub const NIR_SCHEMA_VERSION: u32 = 1;

/// Canonical metadata bound into an internal NIR artifact digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactMetadata {
    compiler_version: String,
    target_profile: String,
    feature_flags: Vec<String>,
}

impl ArtifactMetadata {
    /// Creates canonical compiler-specific artifact metadata.
    ///
    /// Feature flags are sorted so caller collection order cannot affect bytes.
    ///
    /// # Errors
    ///
    /// Rejects empty, non-portable, or duplicate metadata values.
    pub fn new<I, S>(
        compiler_version: impl Into<String>,
        target_profile: impl Into<String>,
        feature_flags: I,
    ) -> Result<Self, ArtifactError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let compiler_version = compiler_version.into();
        let target_profile = target_profile.into();
        validate_metadata("compiler version", &compiler_version)?;
        validate_metadata("target profile", &target_profile)?;
        let mut feature_flags = feature_flags
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        for feature in &feature_flags {
            validate_metadata("feature flag", feature)?;
        }
        feature_flags.sort();
        if feature_flags.windows(2).any(|pair| pair[0] == pair[1]) {
            return artifact_error(
                ArtifactErrorCode::InvalidMetadata,
                "feature flags must be unique".to_owned(),
            );
        }
        Ok(Self {
            compiler_version,
            target_profile,
            feature_flags,
        })
    }

    /// Returns the exact compiler version required to consume the artifact.
    #[must_use]
    pub fn compiler_version(&self) -> &str {
        &self.compiler_version
    }

    /// Returns the explicit target-independent feature/profile identity.
    #[must_use]
    pub fn target_profile(&self) -> &str {
        &self.target_profile
    }

    /// Returns feature flags in canonical lexical order.
    #[must_use]
    pub fn feature_flags(&self) -> &[String] {
        &self.feature_flags
    }
}

/// Exact compatibility expected by a pinned verifier/backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactCompatibility {
    compiler_version: String,
}

impl ArtifactCompatibility {
    /// Requires the current NIR schema and one exact compiler version.
    #[must_use]
    pub fn exact(compiler_version: impl Into<String>) -> Self {
        Self {
            compiler_version: compiler_version.into(),
        }
    }
}

/// Stable category for an internal NIR artifact rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactErrorCode {
    /// Bytes are not strict schema-conforming JSON.
    InvalidJson,
    /// Artifact magic does not identify private Futao NIR.
    InvalidMagic,
    /// NIR schema version is unsupported.
    UnsupportedSchema,
    /// Artifact compiler version does not match the pinned consumer.
    IncompatibleCompiler,
    /// Header metadata is empty, malformed, duplicated, or unsorted.
    InvalidMetadata,
    /// Content digest is malformed.
    InvalidContentHash,
    /// Content bytes do not match the recorded digest.
    ContentHashMismatch,
    /// JSON is valid but not the unique canonical byte encoding.
    NonCanonicalEncoding,
    /// The contained module fails independent NIR verification.
    Verification,
    /// A verified artifact could not be encoded.
    Serialization,
}

impl ArtifactErrorCode {
    /// Returns the stable artifact diagnostic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidJson => "E5110",
            Self::InvalidMagic => "E5111",
            Self::UnsupportedSchema => "E5112",
            Self::IncompatibleCompiler => "E5113",
            Self::InvalidMetadata => "E5114",
            Self::InvalidContentHash => "E5115",
            Self::ContentHashMismatch => "E5116",
            Self::NonCanonicalEncoding => "E5117",
            Self::Verification => "E5118",
            Self::Serialization => "E5119",
        }
    }
}

/// One deterministic internal artifact failure.
#[derive(Debug, Error)]
#[error("{}: {message}", code.as_str())]
pub struct ArtifactError {
    code: ArtifactErrorCode,
    message: String,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ArtifactError {
    /// Returns the stable failure category.
    #[must_use]
    pub const fn code(&self) -> ArtifactErrorCode {
        self.code
    }

    /// Returns deterministic failure context.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DigestInput<'a> {
    nir_schema_version: u32,
    compiler_version: &'a str,
    target_profile: &'a str,
    feature_flags: &'a [String],
    module: &'a UnverifiedModule,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ArtifactEnvelope {
    magic: String,
    nir_schema_version: u32,
    compiler_version: String,
    target_profile: String,
    feature_flags: Vec<String>,
    content_hash: String,
    module: UnverifiedModule,
}

/// Canonical serializer and strict loader for private compiler NIR artifacts.
#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalArtifact;

impl CanonicalArtifact {
    /// Serializes verified NIR into its unique compiler-private JSON encoding.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactErrorCode::UnsupportedSchema`] when the module uses a
    /// type shape not frozen in schema 1, or [`ArtifactErrorCode::Serialization`]
    /// if encoding fails.
    pub fn serialize(
        module: &VerifiedModule,
        metadata: &ArtifactMetadata,
    ) -> Result<Vec<u8>, ArtifactError> {
        ensure_schema_supported(module.module())?;
        let content_hash = content_hash(module.module(), metadata)?;
        let envelope = ArtifactEnvelope {
            magic: NIR_ARTIFACT_MAGIC.to_owned(),
            nir_schema_version: NIR_SCHEMA_VERSION,
            compiler_version: metadata.compiler_version.clone(),
            target_profile: metadata.target_profile.clone(),
            feature_flags: metadata.feature_flags.clone(),
            content_hash,
            module: module.module().clone(),
        };
        serde_json::to_vec(&envelope).map_err(serialization_error)
    }

    /// Loads, authenticates, independently verifies, and canonicalizes NIR bytes.
    ///
    /// # Errors
    ///
    /// Fails closed for malformed data, unknown versions or fields, digest
    /// mismatch, invalid NIR, and any non-canonical byte representation.
    pub fn deserialize(
        bytes: &[u8],
        compatibility: &ArtifactCompatibility,
    ) -> Result<VerifiedModule, ArtifactError> {
        let envelope: ArtifactEnvelope =
            serde_json::from_slice(bytes).map_err(|source| ArtifactError {
                code: ArtifactErrorCode::InvalidJson,
                message: "artifact does not match the strict NIR JSON schema".to_owned(),
                source: Some(Box::new(source)),
            })?;
        if envelope.magic != NIR_ARTIFACT_MAGIC {
            return artifact_error(
                ArtifactErrorCode::InvalidMagic,
                format!("unexpected artifact magic `{}`", envelope.magic),
            );
        }
        if envelope.nir_schema_version != NIR_SCHEMA_VERSION {
            return artifact_error(
                ArtifactErrorCode::UnsupportedSchema,
                format!(
                    "NIR schema {} is unsupported; expected {}",
                    envelope.nir_schema_version, NIR_SCHEMA_VERSION
                ),
            );
        }
        ensure_schema_supported(&envelope.module)?;
        if envelope.compiler_version != compatibility.compiler_version {
            return artifact_error(
                ArtifactErrorCode::IncompatibleCompiler,
                format!(
                    "artifact compiler {} does not match pinned compiler {}",
                    envelope.compiler_version, compatibility.compiler_version
                ),
            );
        }
        if !valid_sha256(&envelope.content_hash) {
            return artifact_error(
                ArtifactErrorCode::InvalidContentHash,
                "contentHash must be an algorithm-qualified lowercase SHA-256 digest".to_owned(),
            );
        }
        let metadata = ArtifactMetadata::new(
            envelope.compiler_version.clone(),
            envelope.target_profile.clone(),
            envelope.feature_flags.clone(),
        )?;
        if metadata.feature_flags != envelope.feature_flags {
            return artifact_error(
                ArtifactErrorCode::InvalidMetadata,
                "feature flags are not in canonical lexical order".to_owned(),
            );
        }
        let actual_hash = content_hash(&envelope.module, &metadata)?;
        if actual_hash != envelope.content_hash {
            return artifact_error(
                ArtifactErrorCode::ContentHashMismatch,
                format!(
                    "content hash mismatch: expected {}, found {actual_hash}",
                    envelope.content_hash
                ),
            );
        }
        let verified = Verifier::verify(envelope.module).map_err(verification_error)?;
        let canonical = Self::serialize(&verified, &metadata)?;
        if canonical != bytes {
            return artifact_error(
                ArtifactErrorCode::NonCanonicalEncoding,
                "artifact bytes are not the canonical JSON encoding".to_owned(),
            );
        }
        Ok(verified)
    }
}

fn ensure_schema_supported(module: &UnverifiedModule) -> Result<(), ArtifactError> {
    if module
        .types
        .iter()
        .any(|definition| matches!(&definition.ty, NirType::TaggedUnion { .. }))
    {
        return artifact_error(
            ArtifactErrorCode::UnsupportedSchema,
            format!("NIR schema {NIR_SCHEMA_VERSION} does not encode tagged-union types"),
        );
    }
    Ok(())
}

fn content_hash(
    module: &UnverifiedModule,
    metadata: &ArtifactMetadata,
) -> Result<String, ArtifactError> {
    let input = DigestInput {
        nir_schema_version: NIR_SCHEMA_VERSION,
        compiler_version: &metadata.compiler_version,
        target_profile: &metadata.target_profile,
        feature_flags: &metadata.feature_flags,
        module,
    };
    let bytes = serde_json::to_vec(&input).map_err(serialization_error)?;
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity("sha256:".len() + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        write!(encoded, "{byte:02x}").map_err(|source| ArtifactError {
            code: ArtifactErrorCode::Serialization,
            message: "failed to encode the content hash".to_owned(),
            source: Some(Box::new(source)),
        })?;
    }
    Ok(encoded)
}

fn validate_metadata(field: &str, value: &str) -> Result<(), ArtifactError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if !valid {
        return artifact_error(
            ArtifactErrorCode::InvalidMetadata,
            format!("{field} `{value}` is not a portable metadata atom"),
        );
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn serialization_error(source: serde_json::Error) -> ArtifactError {
    ArtifactError {
        code: ArtifactErrorCode::Serialization,
        message: "failed to serialize canonical NIR JSON".to_owned(),
        source: Some(Box::new(source)),
    }
}

fn verification_error(source: VerificationError) -> ArtifactError {
    ArtifactError {
        code: ArtifactErrorCode::Verification,
        message: format!("contained NIR failed verification: {source}"),
        source: Some(Box::new(source)),
    }
}

fn artifact_error<T>(code: ArtifactErrorCode, message: String) -> Result<T, ArtifactError> {
    Err(ArtifactError {
        code,
        message,
        source: None,
    })
}
