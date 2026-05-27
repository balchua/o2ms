package com.example.demo;

import static org.springframework.security.test.web.servlet.request.SecurityMockMvcRequestPostProcessors.jwt;
import static org.springframework.security.test.web.servlet.setup.SecurityMockMvcConfigurers.springSecurity;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import java.util.List;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.setup.MockMvcBuilders;
import org.springframework.web.context.WebApplicationContext;

@SpringBootTest
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
}
