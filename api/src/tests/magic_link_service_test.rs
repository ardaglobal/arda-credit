#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::magic_link_service::{MagicLinkService, MagicLinkConfig, MagicLinkError};
    use std::sync::Arc;
    use tokio_test;

    async fn create_test_service() -> MagicLinkService {
        // This would require setting up test database and mock services
        // Implementation depends on your test infrastructure
        todo!("Implement test setup")
    }

    #[tokio::test]
    async fn test_token_generation() {
        use crate::services::magic_link_service::MagicLinkService;

        // Test token generation
        let token1 = MagicLinkService::generate_token();
        let token2 = MagicLinkService::generate_token();

        // Tokens should be different
        assert_ne!(token1, token2);

        // Tokens should be correct length (32 bytes base64url = 43 chars)
        assert_eq!(token1.len(), 43);
        assert_eq!(token2.len(), 43);

        // Tokens should be URL safe
        assert!(!token1.contains('+'));
        assert!(!token1.contains('/'));
        assert!(!token1.contains('='));
    }

    #[tokio::test]
    async fn test_state_generation() {
        use crate::services::magic_link_service::MagicLinkService;

        let state1 = MagicLinkService::generate_state();
        let state2 = MagicLinkService::generate_state();

        // States should be different
        assert_ne!(state1, state2);

        // States should be correct length (16 bytes base64url = 22 chars)
        assert_eq!(state1.len(), 22);
        assert_eq!(state2.len(), 22);
    }

    #[tokio::test]
    async fn test_token_hashing() {
        use crate::services::magic_link_service::MagicLinkService;

        let token = "test-token-123";
        let hash1 = MagicLinkService::hash_token(token);
        let hash2 = MagicLinkService::hash_token(token);

        // Same token should produce same hash
        assert_eq!(hash1, hash2);

        // Hash should be 32 bytes (SHA-256)
        assert_eq!(hash1.len(), 32);

        // Different tokens should produce different hashes
        let hash3 = MagicLinkService::hash_token("different-token");
        assert_ne!(hash1, hash3);
    }

    #[tokio::test]
    async fn test_magic_link_url_creation() {
        let config = MagicLinkConfig {
            base_url: "https://app.arda.xyz/auth/verify".to_string(),
            ..Default::default()
        };

        let service = create_test_service().await;
        let token = "test-token";
        let state = "test-state";

        let url = service.create_magic_link_url(token, state);

        assert_eq!(url, "https://app.arda.xyz/auth/verify?token=test-token&state=test-state");
    }
}
