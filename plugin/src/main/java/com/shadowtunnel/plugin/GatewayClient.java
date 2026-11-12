package com.shadowtunnel.plugin;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.concurrent.CompletableFuture;
import java.util.regex.Pattern;

final class GatewayClient {
    private final HttpClient client = HttpClient.newHttpClient();
    private final String baseUrl;
    private final String token;

    GatewayClient(String baseUrl, String token) {
        this.baseUrl = baseUrl.replaceAll("/$", "");
        this.token = token;
    }

    CompletableFuture<String> status(String name) { return request("GET", "/api/tunnels/" + encode(name) + "/status", null); }
    CompletableFuture<String> connect(String name) { return request("POST", "/api/tunnels/" + encode(name) + "/connect", ""); }
    CompletableFuture<String> disconnect(String name) { return request("POST", "/api/tunnels/" + encode(name) + "/disconnect", ""); }
    CompletableFuture<String> create(String name, String type, String destination) {
        String body = "{\"tunnel\":\"" + json(name) + "\",\"local_type\":\"" + json(type) + "\",\"local_destination\":\"" + json(destination) + "\"}";
        return request("POST", "/api/tunnels", body);
    }
    private CompletableFuture<String> request(String method, String path, String body) {
        HttpRequest.Builder builder = HttpRequest.newBuilder(URI.create(baseUrl + path)).timeout(Duration.ofSeconds(10)).header("SHADOW_API_TOKEN", token);
        if (body == null) builder.GET(); else builder.method(method, HttpRequest.BodyPublishers.ofString(body)).header("Content-Type", "application/json");
        return client.sendAsync(builder.build(), HttpResponse.BodyHandlers.ofString()).thenCompose(response -> response.statusCode() >= 200 && response.statusCode() < 300 ? CompletableFuture.completedFuture(response.body()) : CompletableFuture.failedFuture(new IllegalStateException("HTTP " + response.statusCode())));
    }
    private static String encode(String value) { return value.replace("%", "%25").replace("/", "%2F"); }
    private static String json(String value) { return value.replace("\\", "\\\\").replace("\"", "\\\""); }
    static String field(String json, String field) { var match = Pattern.compile("\\\"" + field + "\\\"\\s*:\\s*(?:\\\"([^\\\"]*)\\\"|(true|false|null))").matcher(json); return match.find() ? (match.group(1) != null ? match.group(1) : match.group(2)) : "unknown"; }
}
