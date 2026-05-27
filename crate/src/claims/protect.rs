pub const PROTECTED_CLAIMS: &[&str] = &["iss", "aud", "exp", "iat", "jti"];

#[cfg(test)]
mod tests {
    use super::PROTECTED_CLAIMS;

    #[test]
    fn protected_claims_include_standard_fields() {
        assert!(PROTECTED_CLAIMS.contains(&"iss"));
        assert!(PROTECTED_CLAIMS.contains(&"jti"));
    }
}
