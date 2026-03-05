package com.falcraft.commands;

import com.falcraft.util.BlockMapper;
import com.falcraft.util.FalAPI;
import com.falcraft.util.GLBParser;
import com.falcraft.util.MeshSimplifier;
import com.falcraft.util.PlacementPreview;
import com.falcraft.util.TextureSampler;
import com.falcraft.util.Voxelizer;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.suggestion.SuggestionProvider;
import net.minecraft.ChatFormatting;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.client.Minecraft;
import net.minecraft.core.BlockPos;
import net.minecraft.network.chat.ClickEvent;
import net.minecraft.network.chat.Component;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.HashMap;
import java.util.Map;

import static net.fabricmc.fabric.api.client.command.v2.ClientCommandManager.argument;
import static net.fabricmc.fabric.api.client.command.v2.ClientCommandManager.literal;

/**
 * Command to generate 3D models using Nano Banana 2 + SAM-3D, then remap to wool only.
 * Usage:
 * - /woolgen <size> <prompt>
 * - /woolgenstream <size> <prompt>
 * - /fal woolgen <size> <prompt>
 * - /fal woolgenstream <size> <prompt>
 */
public class WoolGenCommand {
    private static final Logger LOGGER = LoggerFactory.getLogger("WoolGenCommand");
    private static final int STREAM_LAYERS_PER_BATCH = 1;
    private static final int STREAM_TICKS_BETWEEN_BATCHES = 20;

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
        dispatcher.register(literal("woolgen")
                .then(argument("size", IntegerArgumentType.integer(16, 128))
                        .suggests(SIZE_SUGGESTIONS)
                        .then(argument("prompt", StringArgumentType.greedyString())
                                .executes(WoolGenCommand::execute))));

        dispatcher.register(literal("woolgenstream")
                .then(argument("size", IntegerArgumentType.integer(16, 128))
                        .suggests(SIZE_SUGGESTIONS)
                        .then(argument("prompt", StringArgumentType.greedyString())
                                .executes(WoolGenCommand::executeStream))));

        dispatcher.register(literal("fal")
                .then(literal("woolgen")
                        .then(argument("size", IntegerArgumentType.integer(16, 128))
                                .suggests(SIZE_SUGGESTIONS)
                                .then(argument("prompt", StringArgumentType.greedyString())
                                        .executes(WoolGenCommand::execute))))
                .then(literal("woolgenstream")
                        .then(argument("size", IntegerArgumentType.integer(16, 128))
                                .suggests(SIZE_SUGGESTIONS)
                                .then(argument("prompt", StringArgumentType.greedyString())
                                        .executes(WoolGenCommand::executeStream)))));
    }

    private static int execute(CommandContext<FabricClientCommandSource> context) {
        return execute(context, false);
    }

    private static int executeStream(CommandContext<FabricClientCommandSource> context) {
        return execute(context, true);
    }

    private static int execute(CommandContext<FabricClientCommandSource> context, boolean streamPlacement) {
        int size = IntegerArgumentType.getInteger(context, "size");
        String prompt = StringArgumentType.getString(context, "prompt");
        FabricClientCommandSource source = context.getSource();
        String commandName = streamPlacement ? "woolgenstream" : "woolgen";

        source.sendFeedback(Component.literal("§e[fal] Starting wool-only 3D generation (" + size + "x" + size + "x" + size + ")"));
        source.sendFeedback(Component.literal("§e[fal] Prompt: \"" + prompt + "\""));
        source.sendFeedback(Component.literal("§e[fal] Pipeline: Nano Banana 2 + SAM-3D + wool palette remap"));
        if (streamPlacement) {
            source.sendFeedback(Component.literal("§e[fal] Build mode: 3D-printer stream (1 layer/sec)"));
        }

        new Thread(() -> {
            try {
                LOGGER.info("Starting wool-only Nano Banana 2 3D generation process...");

                Minecraft.getInstance().execute(() ->
                        source.sendFeedback(Component.literal("§e[fal] [1/5] Generating 2D image (Nano Banana 2)...")));

                FalAPI falApi = new FalAPI();
                FalAPI.ModelWithImageResult modelWithImage = falApi.generateModelWithNanoBanana2WithImage(prompt);
                FalAPI.ModelResult modelResult = modelWithImage.modelResult();
                String sourceImageUrl = modelWithImage.imageUrl();
                String modelGlbUrl = modelResult.glbUrl();

                LOGGER.info("Received GLB model ({} bytes)", modelResult.glbData().length);
                Minecraft.getInstance().execute(() -> {
                    source.sendFeedback(Component.literal("§e[fal] [2/5] 3D model generated! Processing..."));
                    source.sendFeedback(Component.literal("§7[fal] Generated image URL:"));
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
                        source.sendFeedback(Component.literal("§e[fal] [3/5] Parsing 3D model...")));

                GLBParser.MeshData meshData = GLBParser.parse(modelResult.glbData(), textureSampler);

                int sourceTriangles = meshData.indices().length / 3;
                int simplifyTargetTriangles = getSimplifyTargetTriangles(size);
                GLBParser.MeshData voxelMesh = meshData;
                if (sourceTriangles > simplifyTargetTriangles) {
                    Minecraft.getInstance().execute(() ->
                            source.sendFeedback(Component.literal("§e[fal] [4/5] Simplifying mesh for voxelization...")));

                    voxelMesh = MeshSimplifier.simplify(meshData, simplifyTargetTriangles);
                    int simplifiedTriangles = voxelMesh.indices().length / 3;
                    LOGGER.info("Woolgen mesh simplified from {} to {} triangles (target={})",
                            sourceTriangles, simplifiedTriangles, simplifyTargetTriangles);

                    int finalSimplifiedTriangles = simplifiedTriangles;
                    Minecraft.getInstance().execute(() ->
                            source.sendFeedback(Component.literal("§7[fal] Mesh simplified: "
                                    + sourceTriangles + " -> " + finalSimplifiedTriangles + " triangles")));
                } else {
                    LOGGER.info("Woolgen mesh simplification skipped ({} triangles <= target {})",
                            sourceTriangles, simplifyTargetTriangles);
                }

                final TextureSampler finalTextureSampler = textureSampler;
                Minecraft.getInstance().execute(() ->
                        source.sendFeedback(Component.literal("§e[fal] [4/5] Converting to voxels...")));

                Voxelizer.VoxelGrid baseGrid = Voxelizer.voxelize(voxelMesh, size, finalTextureSampler);

                if (baseGrid.voxels().isEmpty()) {
                    Minecraft.getInstance().execute(() ->
                            source.sendError(Component.literal("§c[fal] Error: Generated model has no voxels!")));
                    return;
                }

                Minecraft.getInstance().execute(() ->
                        source.sendFeedback(Component.literal("§e[fal] [5/5] Remapping voxels to colored wool...")));
                Voxelizer.VoxelGrid woolGrid = remapToWoolPalette(baseGrid);

                Minecraft.getInstance().execute(() -> {
                    try {
                        if (streamPlacement) {
                            PlacementPreview.startPlacement(
                                    woolGrid,
                                    STREAM_LAYERS_PER_BATCH,
                                    STREAM_TICKS_BETWEEN_BATCHES
                            );
                            source.sendFeedback(Component.literal(
                                    "§a[fal] ✓ Woolgen stream complete! " + woolGrid.voxels().size() + " wool blocks ready."));
                            source.sendFeedback(Component.literal(
                                    "§e[fal] Right-click to start layer build (1 layer/sec), G to rotate!"));
                        } else {
                            PlacementPreview.startPlacement(woolGrid);
                            source.sendFeedback(Component.literal(
                                    "§a[fal] ✓ Woolgen complete! " + woolGrid.voxels().size() + " wool blocks ready."));
                            source.sendFeedback(Component.literal(
                                    "§e[fal] Right-click to place, G to rotate!"));
                        }
                    } catch (Exception e) {
                        String errorMsg = e.getMessage();
                        source.sendError(Component.literal("§c[fal] Error preparing placement: " + errorMsg));
                        LOGGER.error("Error preparing wool placement", e);
                    }
                });

            } catch (IllegalStateException e) {
                Minecraft.getInstance().execute(() ->
                        source.sendError(Component.literal("§c[fal] Error: API key not configured. Use /fal setkey <key>")));
                LOGGER.error("API key not configured", e);
            } catch (Exception e) {
                String errorMsg = e.getMessage();
                Minecraft.getInstance().execute(() ->
                        source.sendError(Component.literal("§c[fal] Error during " + commandName + ": " + errorMsg)));
                LOGGER.error("Error during wool-only Nano Banana 2 generation process", e);
            }
        }, streamPlacement ? "fal-WoolGenStream-Thread" : "fal-WoolGen-Thread").start();

        return 1;
    }

    private static Voxelizer.VoxelGrid remapToWoolPalette(Voxelizer.VoxelGrid grid) {
        Map<BlockPos, Integer> woolVoxels = new HashMap<>(grid.voxels().size());
        for (Map.Entry<BlockPos, Integer> entry : grid.voxels().entrySet()) {
            int woolColor = BlockMapper.getClosestWoolColor(entry.getValue());
            woolVoxels.put(entry.getKey(), woolColor);
        }
        return new Voxelizer.VoxelGrid(woolVoxels, grid.size());
    }

    /**
     * Mesh simplification target tuned by requested voxel grid size.
     * Smaller grids need fewer triangles, larger grids can preserve more detail.
     */
    private static int getSimplifyTargetTriangles(int gridSize) {
        int scaled = gridSize * gridSize * 20;
        return Math.max(12_000, Math.min(80_000, scaled));
    }
}
