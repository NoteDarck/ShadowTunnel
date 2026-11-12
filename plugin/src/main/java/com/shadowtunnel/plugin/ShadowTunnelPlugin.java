package com.shadowtunnel.plugin;

import org.bukkit.ChatColor;
import org.bukkit.command.*;
import org.bukkit.plugin.java.JavaPlugin;
import java.util.List;
import java.util.concurrent.CompletableFuture;

public final class ShadowTunnelPlugin extends JavaPlugin implements CommandExecutor, TabCompleter {
    private GatewayClient gateway;
    private String tunnel;
    @Override public void onEnable() {
        saveDefaultConfig();
        tunnel = getConfig().getString("tunnel.name", "default");
        gateway = new GatewayClient(getConfig().getString("api-url", "http://127.0.0.1:9100"), getConfig().getString("api-token", ""));
        PluginCommand command = getCommand("shadowtunnel");
        if (command == null) {
            getLogger().severe("Comando shadowtunnel não registrado");
            getServer().getPluginManager().disablePlugin(this);
            return;
        }
        command.setExecutor(this);
        command.setTabCompleter(this);
    }
    @Override public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (args.length != 1 || !List.of("status", "connect", "disconnect", "info").contains(args[0].toLowerCase())) return false;
        String action = args[0].toLowerCase(); sender.sendMessage(ChatColor.YELLOW + "ShadowTunnel: request sent");
        CompletableFuture<String> request = switch (action) { case "status", "info" -> gateway.status(tunnel); case "connect" -> gateway.create(tunnel, getConfig().getString("local-type", "java"), getConfig().getString("local-destination", "127.0.0.1:25565")).thenCompose(ignored -> gateway.connect(tunnel)); default -> gateway.disconnect(tunnel); };
        request.thenAccept(body -> getServer().getScheduler().runTask(this, () -> show(sender, action, body))).exceptionally(error -> {
            Throwable cause = error.getCause() == null ? error : error.getCause();
            getServer().getScheduler().runTask(this, () -> sender.sendMessage(ChatColor.RED + "ShadowTunnel request failed: " + cause.getMessage()));
            return null;
        });
        return true;
    }
    private void show(CommandSender sender, String action, String body) {
        sender.sendMessage(ChatColor.GREEN + "tunnel=" + GatewayClient.field(body, "tunnel") + " online=" + GatewayClient.field(body, "online") + " requested=" + GatewayClient.field(body, "requested"));
        if (action.equals("info")) sender.sendMessage(ChatColor.GREEN + "tcp=" + GatewayClient.field(body, "tcp_address") + " udp=" + GatewayClient.field(body, "udp_address"));
    }
    @Override public List<String> onTabComplete(CommandSender sender, Command command, String alias, String[] args) { return args.length == 1 ? List.of("status", "connect", "disconnect", "info").stream().filter(x -> x.startsWith(args[0].toLowerCase())).toList() : List.of(); }
}
