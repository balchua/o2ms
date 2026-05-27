package com.example.demo;

import java.util.LinkedHashMap;
import java.util.Map;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.security.oauth2.jwt.Jwt;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class ApiController {

    @GetMapping("/api/me")
    public Map<String, Object> me(@AuthenticationPrincipal Jwt jwt) {
        Map<String, Object> response = new LinkedHashMap<>();
        response.put("subject", jwt.getSubject());
        response.put("issuer", jwt.getIssuer() != null ? jwt.getIssuer().toString() : null);
        response.put("scope", jwt.getClaimAsString("scope"));
        response.put("roles", jwt.getClaimAsStringList("roles"));
        response.put("authorizations", jwt.getClaim("authorizations"));
        return response;
    }

    @GetMapping("/api/ping")
    public Map<String, Object> ping(@AuthenticationPrincipal Jwt jwt) {
        Map<String, Object> response = new LinkedHashMap<>();
        response.put("status", "ok");
        response.put("subject", jwt.getSubject());
        response.put("message", "Bearer token accepted by Spring Security resource server");
        return response;
    }
}
