package com.example.gatewayclient;

import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.CommandLineRunner;
import org.springframework.stereotype.Component;

@Component
public class GatewayClientRunner implements CommandLineRunner {

    private static final Logger log = LoggerFactory.getLogger(GatewayClientRunner.class);

    private final String mockServerBaseUrl;
    private final String gatewayRouteUrl;
    private final String clientId;
    private final String clientSecret;
    private final String scope;

    public GatewayClientRunner(
            @Value("${mock.server.base-url:http://127.0.0.1:8090}") String mockServerBaseUrl,
            @Value("${mock.server.gateway-route:http://127.0.0.1:8090/proxy/springboot/api/me}") String gatewayRouteUrl,
            @Value("${mock.server.client-id:springboot-resource-server}") String clientId,
            @Value("${mock.server.client-secret:abc1234}") String clientSecret,
            @Value("${mock.server.scope:openid}") String scope) {
        this.mockServerBaseUrl = mockServerBaseUrl;
        this.gatewayRouteUrl = gatewayRouteUrl;
        this.clientId = clientId;
        this.clientSecret = clientSecret;
        this.scope = scope;
    }

    @Override
    public void run(String... args) throws Exception {
        HttpClient client = HttpClient.newHttpClient();

        // Step 1: Obtain a token from the mock server using client_secret_basic
        log.info("Obtaining token from {}/token ...", mockServerBaseUrl);
        String tokenBody = "grant_type=client_credentials"
                + "&client_id=" + urlEncode(clientId)
                + "&scope=" + urlEncode(scope);

        String basicAuth = Base64.getEncoder().encodeToString(
                (clientId + ":" + clientSecret).getBytes(StandardCharsets.UTF_8));

        HttpRequest tokenRequest = HttpRequest.newBuilder()
                .uri(URI.create(mockServerBaseUrl + "/token"))
                .header("Content-Type", "application/x-www-form-urlencoded")
                .header("Authorization", "Basic " + basicAuth)
                .POST(HttpRequest.BodyPublishers.ofString(tokenBody))
                .build();

        HttpResponse<String> tokenResponse = client.send(tokenRequest, HttpResponse.BodyHandlers.ofString());

        if (tokenResponse.statusCode() != 200) {
            log.error("Failed to obtain token: {} {}", tokenResponse.statusCode(), tokenResponse.body());
            return;
        }

        // Extract access_token from JSON response (simple parsing, no Jackson dependency needed)
        String responseBody = tokenResponse.body();
        String accessToken = extractJsonString(responseBody, "access_token");
        if (accessToken == null) {
            log.error("Could not extract access_token from response: {}", responseBody);
            return;
        }
        log.info("Token obtained successfully");

        // Step 2: Call the gateway route with the token
        log.info("Calling gateway at {} ...", gatewayRouteUrl);
        HttpRequest gatewayRequest = HttpRequest.newBuilder()
                .uri(URI.create(gatewayRouteUrl))
                .header("Authorization", "Bearer " + accessToken)
                .GET()
                .build();

        HttpResponse<String> gatewayResponse = client.send(gatewayRequest, HttpResponse.BodyHandlers.ofString());

        log.info("Gateway response status: {}", gatewayResponse.statusCode());
        log.info("Gateway response body: {}", gatewayResponse.body());

        // Step 3: Demonstrate auth rejection (call without token)
        log.info("Calling gateway without token (expect 401) ...");
        HttpRequest noAuthRequest = HttpRequest.newBuilder()
                .uri(URI.create(gatewayRouteUrl))
                .GET()
                .build();

        HttpResponse<String> noAuthResponse = client.send(noAuthRequest, HttpResponse.BodyHandlers.ofString());
        log.info("Gateway response without token: {} {}", noAuthResponse.statusCode(), noAuthResponse.body());
    }

    private static String urlEncode(String value) {
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }

    private static String extractJsonString(String json, String key) {
        String search = "\"" + key + "\":\"";
        int start = json.indexOf(search);
        if (start < 0) return null;
        start += search.length();
        int end = json.indexOf("\"", start);
        return end < 0 ? null : json.substring(start, end);
    }
}