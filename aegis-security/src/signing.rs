//! Code signing and signature verification for plugins

use anyhow::{Result, Context, bail};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn, debug};

/// Code signer for creating plugin signatures
pub struct CodeSigner {
    signing_key: SigningKey,
    publisher_info: PublisherInfo,
    trust_level: TrustLevel,
}

impl CodeSigner {
    /// Create a new code signer with a new key pair
    pub fn new(publisher_info: PublisherInfo, trust_level: TrustLevel) -> Self {
        use rand::RngCore;
        let mut csprng = rand::rngs::OsRng{};
        let mut secret_key_bytes = [0u8; 32];
        csprng.fill_bytes(&mut secret_key_bytes);
        let signing_key = SigningKey::from_bytes(&secret_key_bytes);
        
        Self {
            signing_key,
            publisher_info,
            trust_level,
        }
    }
    
    /// Create a code signer from existing key material
    pub fn from_signing_key(
        signing_key: SigningKey, 
        publisher_info: PublisherInfo, 
        trust_level: TrustLevel
    ) -> Self {
        Self {
            signing_key,
            publisher_info,
            trust_level,
        }
    }
    
    /// Sign a plugin package
    pub async fn sign_plugin(&self, plugin_path: &Path) -> Result<PluginSignature> {
        info!("Signing plugin: {}", plugin_path.display());
        
        // Calculate comprehensive hash of plugin contents
        let plugin_hash = self.calculate_plugin_hash(plugin_path).await?;
        
        // Create signature payload
        let payload = SignaturePayload {
            plugin_hash: plugin_hash.clone(),
            publisher_id: self.publisher_info.publisher_id.clone(),
            publisher_email: self.publisher_info.email.clone(),
            trust_level: self.trust_level,
            timestamp: chrono::Utc::now(),
            aegis_version: env!("CARGO_PKG_VERSION").to_string(),
            signing_algorithm: "ed25519".to_string(),
        };
        
        // Serialize payload for signing
        let payload_bytes = self.serialize_payload(&payload)?;
        
        // Create signature
        let signature = self.signing_key.sign(&payload_bytes);
        
        let plugin_signature = PluginSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            signature: signature.to_bytes().to_vec(),
            public_key: self.signing_key.verifying_key().to_bytes().to_vec(),
            payload,
            publisher_info: self.publisher_info.clone(),
            signature_metadata: SignatureMetadata {
                created_at: chrono::Utc::now(),
                version: 1,
                chain_of_trust: self.build_chain_of_trust(),
            },
        };
        
        debug!("Plugin signature created successfully");
        Ok(plugin_signature)
    }
    
    /// Calculate cryptographic hash of entire plugin
    async fn calculate_plugin_hash(&self, plugin_path: &Path) -> Result<String> {
        let mut hasher = Sha256::new();
        
        if plugin_path.is_file() {
            // Single file plugin
            let content = tokio::fs::read(plugin_path).await?;
            hasher.update(&content);
        } else if plugin_path.is_dir() {
            // Directory plugin - hash all files in deterministic order
            let mut file_paths = Vec::new();
            self.collect_plugin_files(plugin_path, &mut file_paths)?;
            file_paths.sort(); // Ensure deterministic ordering
            
            for file_path in file_paths {
                // Hash file path (relative to plugin root)
                let relative_path = file_path.strip_prefix(plugin_path)
                    .unwrap_or(&file_path);
                hasher.update(relative_path.to_string_lossy().as_bytes());
                
                // Hash file content
                let content = tokio::fs::read(&file_path).await
                    .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
                hasher.update(&content);
            }
        } else {
            bail!("Plugin path is neither file nor directory: {}", plugin_path.display());
        }
        
        Ok(format!("{:x}", hasher.finalize()))
    }
    
    fn collect_plugin_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                // Skip hidden directories and target directories
                let dir_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                
                if !dir_name.starts_with('.') && dir_name != "target" {
                    self.collect_plugin_files(&path, files)?;
                }
            }
        }
        Ok(())
    }
    
    fn serialize_payload(&self, payload: &SignaturePayload) -> Result<Vec<u8>> {
        // Create canonical serialization for consistent signing
        let json = serde_json::to_string(payload)?;
        Ok(json.into_bytes())
    }
    
    fn build_chain_of_trust(&self) -> ChainOfTrust {
        ChainOfTrust {
            root_authority: "Aegis-Assets CA".to_string(),
            intermediate_authorities: vec![],
            publisher_verification: self.publisher_info.verification_status.clone(),
        }
    }
}

/// Signature verifier for validating plugin signatures
pub struct SignatureVerifier {
    trusted_publishers: HashMap<String, TrustedPublisher>,
    revoked_keys: std::collections::HashSet<String>,
    verification_config: VerificationConfig,
}

impl SignatureVerifier {
    /// Create a new signature verifier
    pub fn new() -> Result<Self> {
        Ok(Self {
            trusted_publishers: HashMap::new(),
            revoked_keys: std::collections::HashSet::new(),
            verification_config: VerificationConfig::default(),
        })
    }
    
    /// Add a trusted publisher
    pub fn add_trusted_publisher(&mut self, publisher: TrustedPublisher) {
        self.trusted_publishers.insert(publisher.publisher_id.clone(), publisher);
    }
    
    /// Revoke a public key
    pub fn revoke_key(&mut self, public_key_hex: String) {
        self.revoked_keys.insert(public_key_hex);
    }
    
    /// Verify a plugin signature
    pub async fn verify_plugin(&self, plugin_path: &Path) -> Result<VerificationResult> {
        // Look for signature file
        let signature_path = self.find_signature_file(plugin_path)?;
        
        // Load and parse signature
        let signature_content = tokio::fs::read_to_string(&signature_path).await?;
        let signature: PluginSignature = serde_json::from_str(&signature_content)?;
        
        // Perform verification
        self.verify_signature(&signature, plugin_path).await
    }
    
    /// Verify a plugin signature object
    pub async fn verify_signature(&self, signature: &PluginSignature, plugin_path: &Path) -> Result<VerificationResult> {
        info!("Verifying plugin signature for: {}", plugin_path.display());
        
        // 1. Check if key is revoked
        let public_key_hex = hex::encode(&signature.public_key);
        if self.revoked_keys.contains(&public_key_hex) {
            warn!("Signature uses revoked key: {}", public_key_hex);
            return Ok(VerificationResult::RevokedKey);
        }
        
        // 2. Verify signature timestamp
        let max_age = chrono::Duration::hours(self.verification_config.max_signature_age_hours);
        let signature_age = chrono::Utc::now().signed_duration_since(signature.payload.timestamp);
        
        if signature_age > max_age {
            warn!("Signature is too old: {} hours", signature_age.num_hours());
            return Ok(VerificationResult::ExpiredSignature);
        }
        
        // 3. Verify cryptographic signature
        if signature.public_key.len() != 32 {
            return Ok(VerificationResult::InvalidSignature);
        }
        let mut pubkey_array = [0u8; 32];
        pubkey_array.copy_from_slice(&signature.public_key);
        let verifying_key = VerifyingKey::from_bytes(&pubkey_array)
            .map_err(|e| anyhow::anyhow!("Invalid public key: {}", e))?;
        
        if signature.signature.len() != 64 {
            return Ok(VerificationResult::InvalidSignature);
        }
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&signature.signature);
        let signature_bytes = Signature::try_from(&sig_array[..])
            .map_err(|e| anyhow::anyhow!("Invalid signature: {}", e))?;
        
        let payload_bytes = serde_json::to_string(&signature.payload)?.into_bytes();
        
        if verifying_key.verify(&payload_bytes, &signature_bytes).is_err() {
            warn!("Cryptographic signature verification failed");
            return Ok(VerificationResult::InvalidSignature);
        }
        
        // 4. Verify plugin hash matches current content
        let current_hash = self.calculate_plugin_hash(plugin_path).await?;
        if current_hash != signature.payload.plugin_hash {
            warn!("Plugin content has been modified since signing");
            return Ok(VerificationResult::ModifiedContent);
        }
        
        // 5. Check publisher trust level
        let trust_verification = self.verify_publisher_trust(&signature.payload.publisher_id, signature.payload.trust_level);
        
        // 6. Check chain of trust
        let chain_verification = self.verify_chain_of_trust(&signature.signature_metadata.chain_of_trust);
        
        let verification_result = match (trust_verification, chain_verification) {
            (true, true) => VerificationResult::Valid {
                publisher_id: signature.payload.publisher_id.clone(),
                trust_level: signature.payload.trust_level,
                verified_at: chrono::Utc::now(),
            },
            (false, _) => VerificationResult::UntrustedPublisher,
            (_, false) => VerificationResult::InvalidChainOfTrust,
        };
        
        info!("Signature verification complete: {:?}", verification_result);
        Ok(verification_result)
    }
    
    fn find_signature_file(&self, plugin_path: &Path) -> Result<PathBuf> {
        let signature_extensions = [".sig", ".signature", ".asc"];
        
        for ext in &signature_extensions {
            let sig_path = plugin_path.with_extension(ext);
            if sig_path.exists() {
                return Ok(sig_path);
            }
        }
        
        // Look for signature file in same directory
        if let Some(parent) = plugin_path.parent() {
            let plugin_name = plugin_path.file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("plugin");
            
            for ext in &signature_extensions {
                let sig_path = parent.join(format!("{}{}", plugin_name, ext));
                if sig_path.exists() {
                    return Ok(sig_path);
                }
            }
        }
        
        bail!("No signature file found for plugin: {}", plugin_path.display());
    }
    
    async fn calculate_plugin_hash(&self, plugin_path: &Path) -> Result<String> {
        // Same implementation as CodeSigner::calculate_plugin_hash
        let mut hasher = Sha256::new();
        
        if plugin_path.is_file() {
            let content = tokio::fs::read(plugin_path).await?;
            hasher.update(&content);
        } else if plugin_path.is_dir() {
            let mut file_paths = Vec::new();
            self.collect_plugin_files(plugin_path, &mut file_paths)?;
            file_paths.sort();
            
            for file_path in file_paths {
                let relative_path = file_path.strip_prefix(plugin_path)
                    .unwrap_or(&file_path);
                hasher.update(relative_path.to_string_lossy().as_bytes());
                
                let content = tokio::fs::read(&file_path).await?;
                hasher.update(&content);
            }
        }
        
        Ok(format!("{:x}", hasher.finalize()))
    }
    
    fn collect_plugin_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                let dir_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                
                if !dir_name.starts_with('.') && dir_name != "target" {
                    self.collect_plugin_files(&path, files)?;
                }
            }
        }
        Ok(())
    }
    
    fn verify_publisher_trust(&self, publisher_id: &str, claimed_trust_level: TrustLevel) -> bool {
        if let Some(trusted_publisher) = self.trusted_publishers.get(publisher_id) {
            // Verify claimed trust level doesn't exceed actual trust level
            trusted_publisher.trust_level >= claimed_trust_level
        } else {
            // Unknown publisher - only allow if verification config permits
            self.verification_config.allow_unknown_publishers
        }
    }
    
    fn verify_chain_of_trust(&self, chain: &ChainOfTrust) -> bool {
        // Implement chain of trust verification
        // This would check certificate chains, revocation lists, etc.
        // For now, basic implementation
        !chain.root_authority.is_empty()
    }
}

// ===== DATA STRUCTURES =====

/// Plugin signature containing all verification data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSignature {
    pub algorithm: SignatureAlgorithm,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
    pub payload: SignaturePayload,
    pub publisher_info: PublisherInfo,
    pub signature_metadata: SignatureMetadata,
}

/// Signature algorithm used
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignatureAlgorithm {
    Ed25519,
    // Future algorithms can be added here
}

/// Data that is actually signed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignaturePayload {
    pub plugin_hash: String,
    pub publisher_id: String,
    pub publisher_email: String,
    pub trust_level: TrustLevel,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub aegis_version: String,
    pub signing_algorithm: String,
}

/// Publisher information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublisherInfo {
    pub publisher_id: String,
    pub name: String,
    pub email: String,
    pub website: Option<String>,
    pub verification_status: VerificationStatus,
}

/// Trust levels for publishers
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    Unverified = 0,
    Community = 1,
    Verified = 2,
    Enterprise = 3,
    CoreTeam = 4,
}

/// Publisher verification status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    Unverified,
    EmailVerified,
    IdentityVerified,
    OrganizationVerified,
}

/// Signature metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureMetadata {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub version: u32,
    pub chain_of_trust: ChainOfTrust,
}

/// Chain of trust information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainOfTrust {
    pub root_authority: String,
    pub intermediate_authorities: Vec<String>,
    pub publisher_verification: VerificationStatus,
}

/// Trusted publisher record
#[derive(Debug, Clone)]
pub struct TrustedPublisher {
    pub publisher_id: String,
    pub public_keys: Vec<Vec<u8>>,
    pub trust_level: TrustLevel,
    pub added_at: chrono::DateTime<chrono::Utc>,
    pub verified_by: String,
}

/// Verification configuration
#[derive(Debug, Clone)]
pub struct VerificationConfig {
    pub allow_unknown_publishers: bool,
    pub require_minimum_trust_level: TrustLevel,
    pub max_signature_age_hours: i64,
    pub require_chain_of_trust: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            allow_unknown_publishers: false,
            require_minimum_trust_level: TrustLevel::Unverified,
            max_signature_age_hours: 24 * 7, // 1 week
            require_chain_of_trust: false,
        }
    }
}

/// Signature verification result
#[derive(Debug, Clone, Serialize)]
pub enum VerificationResult {
    Valid {
        publisher_id: String,
        trust_level: TrustLevel,
        verified_at: chrono::DateTime<chrono::Utc>,
    },
    InvalidSignature,
    UntrustedPublisher,
    RevokedKey,
    ExpiredSignature,
    ModifiedContent,
    InvalidChainOfTrust,
    MissingSignature,
    Unsigned,
}

impl VerificationResult {
    /// Get trust score for this verification result
    pub fn trust_score(&self) -> u32 {
        match self {
            VerificationResult::Valid { trust_level, .. } => {
                match trust_level {
                    TrustLevel::CoreTeam => 100,
                    TrustLevel::Enterprise => 90,
                    TrustLevel::Verified => 80,
                    TrustLevel::Community => 60,
                    TrustLevel::Unverified => 40,
                }
            }
            VerificationResult::UntrustedPublisher => 20,
            VerificationResult::Unsigned => 10,
            _ => 0,
        }
    }
    
    /// Check if verification passed
    pub fn is_valid(&self) -> bool {
        matches!(self, VerificationResult::Valid { .. })
    }
}

/// Utility functions for signature management
impl PluginSignature {
    /// Save signature to file
    pub async fn save_to_file(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }
    
    /// Load signature from file
    pub async fn load_from_file(path: &Path) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let signature = serde_json::from_str(&content)?;
        Ok(signature)
    }
    
    /// Export public key in PEM format
    pub fn export_public_key_pem(&self) -> String {
        let key_b64 = base64::encode(&self.public_key);
        format!("-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----", key_b64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_plugin_signing_and_verification() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_path = temp_dir.path().join("test_plugin.rs");
        
        // Create a test plugin file
        tokio::fs::write(&plugin_path, "fn main() { println!(\"Hello, world!\"); }").await.unwrap();
        
        // Create publisher info
        let publisher_info = PublisherInfo {
            publisher_id: "test_publisher".to_string(),
            name: "Test Publisher".to_string(),
            email: "test@example.com".to_string(),
            website: None,
            verification_status: VerificationStatus::EmailVerified,
        };
        
        // Create signer and sign plugin
        let signer = CodeSigner::new(publisher_info.clone(), TrustLevel::Community);
        let signature = signer.sign_plugin(&plugin_path).await.unwrap();
        
        // Save signature
        let signature_path = plugin_path.with_extension("sig");
        signature.save_to_file(&signature_path).await.unwrap();
        
        // Create verifier and verify signature
        let mut verifier = SignatureVerifier::new().unwrap();
        verifier.add_trusted_publisher(TrustedPublisher {
            publisher_id: publisher_info.publisher_id.clone(),
            public_keys: vec![signature.public_key.clone()],
            trust_level: TrustLevel::Community,
            added_at: chrono::Utc::now(),
            verified_by: "test_system".to_string(),
        });
        
        let verification_result = verifier.verify_plugin(&plugin_path).await.unwrap();
        
        assert!(verification_result.is_valid());
    }
}