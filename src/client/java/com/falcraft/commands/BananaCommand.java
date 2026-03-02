package com.falcraft.commands;

import com.falcraft.util.FalAPI;
import com.falcraft.util.GLBParser;
import com.falcraft.util.PlacementPreview;
import com.falcraft.util.TextureSampler;
import com.falcraft.util.Voxelizer;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.suggestion.SuggestionProvider;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import static net.fabricmc.fabric.api.client.command.v2.ClientCommandManager.argument;
import static net.fabricmc.fabric.api.client.command.v2.ClientCommandManager.literal;

/**
 * Command to generate 3D models using Nano Banana 2 + SAM-3D.
 * Usage:
 * - /banana <size> <prompt>
 * - /fal banana <size> <prompt>
 */
public class BananaCommand {
    private static final Logger LOGGER = LoggerFactory.getLogger("BananaCommand");

    private static final SuggestionProvider<FabricClientCommandSource> SIZE_SUGGESTIONS = (context, builder) -> {
        builder.suggest(16, Component.literal("Tiny"));
        builder.suggest(32, Component.literal("Small"));
        builder.suggest(48, Component.literal("Medium"));
        builder.suggest(64, Component.literal("Large"));
        builder.suggest(80, Component.literal("Extra Large"));
        builder.suggest(96, Component.literal("Huge"));
        builder.suggest(112, Component.literal("Massive"));
        builder.suggest(128, Component.literal("Maximum"));
        return builder.buildFuture();
    };

    public static void register(CommandDispatcher<FabricClientCommandSource> dispatcher) {
        dispatcher.register(literal("banana")
                .then(argument("size", IntegerArgumentType.integer(16, 128))
                        .suggests(SIZE_SUGGESTIONS)
                        .then(argument("prompt", StringArgumentType.greedyString())
                                .executes(BananaCommand::execute))));

        dispatcher.register(literal("fal")
                .then(literal("banana")
                        .then(argument("size", IntegerArgumentType.integer(16, 128))
                                .suggests(SIZE_SUGGESTIONS)
                                .then(argument("prompt", StringArgumentType.greedyString())
                                        .executes(BananaCommand::execute)))));
    }

    private static int execute(CommandContext<FabricClientCommandSource> context) {
        int size = IntegerArgumentType.getInteger(context, "size");
        String prompt = StringArgumentType.getString(context, "prompt");
        FabricClientCommandSource source = context.getSource();

        source.sendFeedback(Component.literal("§e[fal] Starting 3D generation (" + size + "x" + size + "x" + size + ")"));
        source.sendFeedback(Component.literal("§e[fal] Prompt: \"" + prompt + "\""));
        source.sendFeedback(Component.literal("§e[fal] Using Nano Banana 2 + SAM-3D (about 30 seconds)"));

        new Thread(() -> {
            try {
                LOGGER.info("Starting Nano Banana 2 3D generation process...");

                Minecraft.getInstance().execute(() ->
                        source.sendFeedback(Component.literal("§e[fal] [1/4] Generating 2D image (Nano Banana 2)...")));

                FalAPI falApi = new FalAPI();
                FalAPI.ModelResult modelResult = falApi.generateModelWithNanoBanana2(prompt);

                LOGGER.info("Received GLB model ({} bytes)", modelResult.glbData().length);
                Minecraft.getInstance().execute(() ->
                        source.sendFeedback(Component.literal("§e[fal] [2/4] 3D model generated! Processing...")));

                TextureSampler textureSampler = null;
                try {
                    byte[] embeddedTexture = GLBParser.extractEmbeddedTexture(modelResult.glbData());
                    if (embeddedTexture != null) {
                        textureSampler = new TextureSampler(embeddedTexture);
                    }
                } catch (Exception e) {
                    LOGGER.error("Failed to extract embedded texture: {}", e.getMessage(), e);
                }

                Minecraft.getInstance().execute(() ->
                        source.sendFeedback(Component.literal("§e[fal] [3/4] Parsing 3D model...")));

                GLBParser.MeshData meshData = GLBParser.parse(modelResult.glbData(), textureSampler);

                final TextureSampler finalTextureSampler = textureSampler;
                Minecraft.getInstance().execute(() ->
                        source.sendFeedback(Component.literal("§e[fal] [4/4] Converting to voxels ("
                                + size + "x" + size + "x" + size + ")...")));

                Voxelizer.VoxelGrid voxelGrid = Voxelizer.voxelize(meshData, size, finalTextureSampler);

                if (voxelGrid.voxels().isEmpty()) {
                    Minecraft.getInstance().execute(() ->
                            source.sendError(Component.literal("§c[fal] Error: Generated model has no voxels!")));
                    return;
                }

                Minecraft.getInstance().execute(() -> {
                    try {
                        PlacementPreview.startPlacement(voxelGrid);

                        source.sendFeedback(Component.literal(
                                "§a[fal] ✓ Banana generation complete! " + voxelGrid.voxels().size() + " blocks ready."));
                        source.sendFeedback(Component.literal(
                                "§e[fal] Right-click to place, G to rotate!"));
                    } catch (Exception e) {
                        String errorMsg = e.getMessage();
                        source.sendError(Component.literal("§c[fal] Error preparing placement: " + errorMsg));
                        LOGGER.error("Error preparing placement", e);
                    }
                });

            } catch (IllegalStateException e) {
                Minecraft.getInstance().execute(() ->
                        source.sendError(Component.literal("§c[fal] Error: API key not configured. Use /fal setkey <key>")));
                LOGGER.error("API key not configured", e);
            } catch (Exception e) {
                String errorMsg = e.getMessage();
                Minecraft.getInstance().execute(() ->
                        source.sendError(Component.literal("§c[fal] Error during banana generation: " + errorMsg)));
                LOGGER.error("Error during Nano Banana 2 3D generation process", e);
            }
        }, "fal-Banana-Thread").start();

        return 1;
    }
}
