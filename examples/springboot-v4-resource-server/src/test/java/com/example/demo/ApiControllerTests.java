package com.example.demo;

import static org.springframework.security.test.web.servlet.request.SecurityMockMvcRequestPostProcessors.jwt;
import static org.springframework.security.test.web.servlet.request.SecurityMockMvcRequestPostProcessors.oidcLogin;
import static org.springframework.security.test.web.servlet.setup.SecurityMockMvcConfigurers.springSecurity;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.header;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import java.util.List;
import org.hamcrest.Matchers;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.setup.MockMvcBuilders;
import org.springframework.web.context.WebApplicationContext;

@SpringBootTest
@Import(TestSecurityConfig.class)
class ApiControllerTests {

    @Autowired
    private WebApplicationContext context;

    @Test
    void meEndpointReturnsJwtClaims() throws Exception {
        MockMvc mockMvc = MockMvcBuilders.webAppContextSetup(context)
            .apply(springSecurity())
            .build();

        mockMvc.perform(get("/api/me")
                .with(jwt().jwt(jwt -> jwt
                    .subject("demo-user")
                    .claim("scope", "openid profile")
                    .claim("roles", List.of("USER", "ADMIN"))
                    .claim("authorizations", List.of())
                )))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.subject").value("demo-user"))
            .andExpect(jsonPath("$.scope").value("openid profile"))
            .andExpect(jsonPath("$.roles[0]").value("USER"))
            .andExpect(jsonPath("$.authorizations").isArray());
    }

    @Test
    void pingEndpointAcceptsBearerJwt() throws Exception {
        MockMvc mockMvc = MockMvcBuilders.webAppContextSetup(context)
            .apply(springSecurity())
            .build();

        mockMvc.perform(get("/api/ping")
                .with(jwt().jwt(jwt -> jwt.subject("demo-user"))))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.status").value("ok"))
            .andExpect(jsonPath("$.subject").value("demo-user"))
            .andExpect(jsonPath("$.message").value("Bearer token accepted by Spring Security resource server"));
    }

    @Test
    void loginMeEndpointReturnsOidcUserClaims() throws Exception {
        MockMvc mockMvc = MockMvcBuilders.webAppContextSetup(context)
            .apply(springSecurity())
            .build();

        mockMvc.perform(get("/login/me")
                .with(oidcLogin().idToken(token -> token
                    .claim("sub", "demo-user")
                    .claim("email", "demo@example.com")
                    .claim("name", "Demo User")
                    .claim("iss", "http://127.0.0.1:8090")
                )))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.subject").value("demo-user"))
            .andExpect(jsonPath("$.email").value("demo@example.com"))
            .andExpect(jsonPath("$.name").value("Demo User"));
    }

    @Test
    void loginMeRedirectsToOauthLoginWhenUnauthenticated() throws Exception {
        MockMvc mockMvc = MockMvcBuilders.webAppContextSetup(context)
            .apply(springSecurity())
            .build();

        mockMvc.perform(get("/login/me").accept(MediaType.TEXT_HTML))
            .andExpect(status().is3xxRedirection())
            .andExpect(header().string("Location", Matchers.containsString("/oauth2/authorization/mock")));
    }
}
