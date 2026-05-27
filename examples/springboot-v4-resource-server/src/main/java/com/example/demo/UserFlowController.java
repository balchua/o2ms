package com.example.demo;

import java.util.LinkedHashMap;
import java.util.Map;
import org.springframework.security.core.annotation.AuthenticationPrincipal;
import org.springframework.security.oauth2.core.oidc.user.OidcUser;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
public class UserFlowController {

    @GetMapping("/")
    public Map<String, Object> home() {
        Map<String, Object> response = new LinkedHashMap<>();
        response.put("message", "Spring Boot example app for oauth2-mock-server");
        response.put("picker", "The mock server now shows a simple user picker page during browser login.");
        response.put("browserProtectedResource", "/login/me");
        response.put("loginStart", "/oauth2/authorization/mock");
        response.put("bearerApi", "/api/me");
        response.put("bearerApiPing", "/api/ping");
        return response;
    }

    @GetMapping("/login/me")
    public Map<String, Object> me(@AuthenticationPrincipal OidcUser user) {
        Map<String, Object> response = new LinkedHashMap<>();
        response.put("subject", user.getSubject());
        response.put("issuer", user.getIssuer() != null ? user.getIssuer().toString() : null);
        response.put("email", user.getEmail());
        response.put("name", user.getFullName());
        response.put("claims", user.getClaims());
        return response;
    }
}
