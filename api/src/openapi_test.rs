// Test file to validate OpenAPI implementation compiles correctly
use crate::rest::get_openapi_spec;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_generation() {
        let spec = get_openapi_spec();
        assert_eq!(spec.info.title, "Arda Credit API");
        assert_eq!(spec.info.version, "1.0.0");

        // Verify we have some components defined
        assert!(spec.components.is_some());
        let components = spec.components.unwrap();
        assert!(!components.schemas.is_empty());

        // Verify we have some paths defined
        assert!(!spec.paths.paths.is_empty());
    }
}
