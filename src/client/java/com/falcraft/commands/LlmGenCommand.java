package com.falcraft.commands;

import com.falcraft.util.FalAPI;
import com.falcraft.util.GLBParser;
import com.falcraft.util.LLMBlockTexturer;
import com.falcraft.util.PlacementPreview;
import com.falcraft.util.TextureSampler;
import com.falcraft.util.Voxelizer;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.suggestion.SuggestionProvider;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.ClickEvent;
import net.minecraft.network.chat.Component;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import static net.fabricmc.fabric.api.client.command.v2.ClientCommandManager.argument;
import static net.fabricmc.fabric.api.client.command.v2.ClientCommandManager.literal;

/**
 * Generates 3D structures and applies LLM-guided constrained texturing.
 * Usage:
 * - /llmgen <size> <prompt>
 * - /llmgen banana <size> <prompt>   (use Nano Banana 2 for image generation)
 * - /fal llmgen <size> <prompt>
 */
public class LlmGenCommand {
    private static final Logger LOGGER = LoggerFactory.getLogger("LlmGenCommand");

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
        dispatcher.register(literal("llmgen")
                .then(argument("size", IntegerArgumentType.integer(16, 128))
                        .suggests(SIZE_SUGGESTIONS)
                        .then(argument("prompt", StringArgumentType.greedyString())
                                .executes(LlmGenCommand::execute)))
                .then(literal("banana")
                        .then(argument("size", IntegerArgumentType.integer(16, 128))
                                .suggests(SIZE_SUGGESTIONS)
                                .then(argument("prompt", StringArgumentType.greedyString())
                                        .executes(LlmGenCommand::executeBanana)))));

        dispatcher.register(literal("fal")
                .then(literal("llmgen")
                        .then(argument("size", IntegerArgumentType.integer(16, 128))
                                .suggests(SIZE_SUGGESTIONS)
                                .then(argument("prompt", StringArgumentType.greedyString())
                                        .executes(LlmGenCommand::execute)))
                        .then(literal("banana")
                                .then(argument("size", IntegerArgumentType.integer(16, 128))
                                        .suggests(SIZE_SUGGESTIONS)
                                        .then(argument("prompt", StringArgumentType.greedyString())
                                                .executes(LlmGenCommand::executeBanana))))));
    }

    private static int execute(CommandContext<FabricClientCommandSource> context) {
        return executeInternal(context, false);
    }

    private static int executeBanana(CommandContext<FabricClientCommandSource> context) {
        return executeInternal(context, true);
    }

    private static int executeInternal(CommandContext<FabricClientCommandSource> context, boolean useNanoBanana2) {
        int size = IntegerArgumentType.getInteger(context, "size");
        String prompt = StringArgumentType.getString(context, "prompt");
        FabricClientCommandSource source = context.getSource();

        source.sendFeedback(Component.literal("§e[fal] Starting LLM textured generation (" + size + "x" + size + "x" + size + ")"));
        source.sendFeedback(Component.literal("§e[fal] Prompt: \"" + prompt + "\""));
        String imageModel = useNanoBanana2 ? "Nano Banana 2" : "Z-Image";
        source.sendFeedback(Component.literal("§e[fal] Pipeline: " + imageModel + " + SAM-3D + LLM material planning"));

        new Thread(() -> runGeneration(source, size, prompt, useNanoBanana2), "fal-LlmGen-Thread").start();
        return 1;
    }

    private static void runGeneration(FabricClientCommandSource source, int size, String prompt, boolean useNanoBanana2) {
        try {
            Minecraft.getInstance().execute(() ->
                    source.sendFeedback(Component.literal("§e[fal] [1/5] Generating 2D image + 3D model...")));

            FalAPI falApi = new FalAPI();
            FalAPI.ModelWithImageResult modelWithImage = useNanoBanana2
                    ? falApi.generateModelWithNanoBanana2WithImage(prompt)
                    : falApi.generateModelFastWithImage(prompt);
            FalAPI.ModelResult modelResult = modelWithImage.modelResult();
            String sourceImageUrl = modelWithImage.imageUrl();
            String modelGlbUrl = modelResult.glbUrl();
            Minecraft.getInstance().execute(() -> {
                source.sendFeedback(Component.literal("§7[fal] Generated source image URL:"));
                source.sendFeedback(
                        Component.literal(sourceImageUrl)
                                .withStyle(style -> style
                                        .withColor(ChatFormatting.AQUA)
                                        .withUnderlined(true)
                                        .withClickEvent(new ClickEvent(ClickEvent.Action.OPEN_URL, sourceImageUrl)))
                );
                if (modelGlbUrl != null && !modelGlbUrl.isBlank()) {
                    source.sendFeedback(Component.literal("§7[fal] Generated 3D model URL (GLB):"));
                    source.sendFeedback(
                            Component.literal(modelGlbUrl)
                                    .withStyle(style -> style
                                            .withColor(ChatFormatting.GREEN)
                                            .withUnderlined(true)
                                            .withClickEvent(new ClickEvent(ClickEvent.Action.OPEN_URL, modelGlbUrl)))
                    );
                }
            });

            Minecraft.getInstance().execute(() ->
                    source.sendFeedback(Component.literal("§e[fal] [2/5] Extracting embedded textures...")));

            TextureSampler textureSampler = null;
            try {
                byte[] embeddedTexture = GLBParser.extractEmbeddedTexture(modelResult.glbData());
                if (embeddedTexture != null) {
                    textureSampler = new TextureSampler(embeddedTexture);
                }
            } catch (Exception e) {
                LOGGER.warn("Could not extract embedded texture for llmgen: {}", e.getMessage());
            }

            Minecraft.getInstance().execute(() ->
                    source.sendFeedback(Component.literal("§e[fal] [3/5] Parsing 3D model...")));

            GLBParser.MeshData meshData = GLBParser.parse(modelResult.glbData(), textureSampler);

            final TextureSampler finalTextureSampler = textureSampler;
            Minecraft.getInstance().execute(() ->
                    source.sendFeedback(Component.literal("§e[fal] [4/5] Voxelizing model...")));
            Voxelizer.VoxelGrid baseGrid = Voxelizer.voxelize(meshData, size, finalTextureSampler);

            if (baseGrid.voxels().isEmpty()) {
                Minecraft.getInstance().execute(() ->
                        source.sendError(Component.literal("§c[fal] Error: Generated model has no voxels!")));
                return;
            }

            Minecraft.getInstance().execute(() ->
                    source.sendFeedback(Component.literal("§e[fal] [5/5] Planning constrained materials with LLM...")));
            Voxelizer.VoxelGrid texturedGrid = LLMBlockTexturer.applyLlmTexturePlan(
                    baseGrid,
                    prompt,
                    sourceImageUrl,
                    falApi
            );

            Minecraft.getInstance().execute(() -> {
                try {
                    PlacementPreview.startPlacement(texturedGrid);
                    source.sendFeedback(Component.literal(
                            "§a[fal] ✓ LLM generation complete! " + texturedGrid.voxels().size() + " blocks ready."));
                    source.sendFeedback(Component.literal(
                            "§e[fal] Right-click to place, G to rotate!"));
                } catch (Exception e) {
                    source.sendError(Component.literal("§c[fal] Error preparing placement: " + e.getMessage()));
                    LOGGER.error("Error preparing placement for llmgen", e);
                }
            });
        } catch (IllegalStateException e) {
            Minecraft.getInstance().execute(() ->
                    source.sendError(Component.literal("§c[fal] Error: API key not configured. Use /fal setkey <key>")));
            LOGGER.error("API key not configured for llmgen", e);
        } catch (Exception e) {
            Minecraft.getInstance().execute(() ->
                    source.sendError(Component.literal("§c[fal] Error during llmgen: " + e.getMessage())));
            LOGGER.error("Error during llmgen pipeline", e);
        }
    }
}
