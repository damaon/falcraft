package com.falcraft.util;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Applies LLM-guided material planning to an existing voxel grid.
 * The LLM selects a constrained block palette and semantic roles,
 * then deterministic rules assign materials for stable, consistent output.
 */
public class LLMBlockTexturer {
    private static final Logger LOGGER = LoggerFactory.getLogger("LLMBlockTexturer");
    private static final int MAX_BLOCKS = 5;
    private static final List<String> ALLOWED_BLOCK_POOL = initializeAllowedPool();
    private static final Set<String> ALLOWED_BLOCK_SET = new HashSet<>(ALLOWED_BLOCK_POOL);

    private record MaterialRoles(
            Block foundation,
            Block wallPrimary,
            Block wallSecondary,
            Block trim,
            Block accent
    ) {}

    private record StructureStats(
            int voxelCount,
            int surfaceCount,
            int minY,
            int maxY,
            double lowBandRatio,
            double midBandRatio,
            double highBandRatio,
            String colorSummary
    ) {}

    /**
     * Textures the grid using LLM-planned constrained materials and deterministic assignment rules.
     */
    public static Voxelizer.VoxelGrid applyLlmTexturePlan(
            Voxelizer.VoxelGrid baseGrid,
            String userPrompt,
            FalAPI falApi
    ) {
        return applyLlmTexturePlan(baseGrid, userPrompt, null, falApi);
    }

    public static Voxelizer.VoxelGrid applyLlmTexturePlan(
            Voxelizer.VoxelGrid baseGrid,
            String userPrompt,
            String sourceImageUrl,
            FalAPI falApi
    ) {
        if (baseGrid == null || baseGrid.voxels().isEmpty()) {
            return baseGrid;
        }

        StructureStats stats = analyzeStructure(baseGrid);
        MaterialRoles roles = chooseRolesWithFallback(userPrompt, stats, baseGrid, sourceImageUrl, falApi);
        Map<BlockPos, Integer> textured = assignMaterials(baseGrid, roles, stats);
        smoothSurface(textured, 1);

        return new Voxelizer.VoxelGrid(textured, baseGrid.size());
    }

    private static MaterialRoles chooseRolesWithFallback(
            String userPrompt,
            StructureStats stats,
            Voxelizer.VoxelGrid baseGrid,
            String sourceImageUrl,
            FalAPI falApi
    ) {
        try {
            String systemPrompt = buildSystemPrompt();
            String requestPrompt = buildUserPrompt(userPrompt, stats);
            String rawPlan = falApi.generateTextWithLlm(systemPrompt, requestPrompt, sourceImageUrl);
            MaterialRoles parsed = parseRoles(rawPlan, baseGrid);
            if (parsed != null) {
                return parsed;
            }
        } catch (Exception e) {
            LOGGER.warn("LLM texture planning failed, using fallback palette: {}", e.getMessage());
        }
        return fallbackRoles(baseGrid);
    }

    private static String buildSystemPrompt() {
        return """
                You are selecting Minecraft building materials.
                Output STRICT JSON only, no markdown, no explanations.
                Constraints:
                - Use only block IDs from the provided allowed list.
                - Pick at most 5 blocks.
                - Match the exact user prompt semantics (castle, house, ship, plane, etc).
                - If an image is provided, use it as the primary visual source for material style.
                - Keep the palette cohesive; avoid random rainbow mixes unless prompt explicitly asks for colorful output.
                - The result should be easy to decorate later.
                Required JSON format:
                {
                  "palette": ["minecraft:block_id", "..."],
                  "roles": {
                    "foundation": "minecraft:block_id",
                    "wall_primary": "minecraft:block_id",
                    "wall_secondary": "minecraft:block_id",
                    "trim": "minecraft:block_id",
                    "accent": "minecraft:block_id"
                  }
                }
                If unsure, pick a neutral and coherent palette.
                """;
    }

    private static String buildUserPrompt(String userPrompt, StructureStats stats) {
        return "Exact user prompt (verbatim): \"" + userPrompt + "\"\n"
                + "Allowed blocks: " + String.join(", ", ALLOWED_BLOCK_POOL) + "\n"
                + "Structure stats: voxels=" + stats.voxelCount
                + ", surface_voxels=" + stats.surfaceCount
                + ", y_min=" + stats.minY
                + ", y_max=" + stats.maxY
                + ", low_band_ratio=" + String.format("%.3f", stats.lowBandRatio)
                + ", mid_band_ratio=" + String.format("%.3f", stats.midBandRatio)
                + ", high_band_ratio=" + String.format("%.3f", stats.highBandRatio)
                + ", dominant_colors=" + stats.colorSummary + "\n"
                + "Return only JSON.";
    }

    private static MaterialRoles parseRoles(String rawPlan, Voxelizer.VoxelGrid baseGrid) {
        if (rawPlan == null || rawPlan.isBlank()) {
            return null;
        }

        String jsonText = extractJsonObject(rawPlan);
        if (jsonText == null) {
            return null;
        }

        JsonObject root;
        try {
            root = JsonParser.parseString(jsonText).getAsJsonObject();
        } catch (Exception e) {
            LOGGER.warn("Failed to parse LLM plan JSON: {}", e.getMessage());
            return null;
        }

        List<Block> fallbackPalette = deriveFallbackPalette(baseGrid);
        List<Block> selectedPalette = parsePalette(root.getAsJsonArray("palette"));
        if (selectedPalette.isEmpty()) {
            selectedPalette = fallbackPalette;
        }
        if (selectedPalette.size() > MAX_BLOCKS) {
            selectedPalette = selectedPalette.subList(0, MAX_BLOCKS);
        }

        JsonObject rolesJson = root.has("roles") && root.get("roles").isJsonObject()
                ? root.getAsJsonObject("roles")
                : new JsonObject();

        Block foundation = resolveRole(rolesJson, "foundation", selectedPalette, 0);
        Block wallPrimary = resolveRole(rolesJson, "wall_primary", selectedPalette, 0);
        Block wallSecondary = resolveRole(rolesJson, "wall_secondary", selectedPalette, 1);
        Block trim = resolveRole(rolesJson, "trim", selectedPalette, Math.min(2, selectedPalette.size() - 1));
        Block accent = resolveRole(rolesJson, "accent", selectedPalette, Math.min(3, selectedPalette.size() - 1));

        if (foundation == null || wallPrimary == null || trim == null) {
            return null;
        }

        return new MaterialRoles(foundation, wallPrimary, wallSecondary, trim, accent);
    }

    private static String extractJsonObject(String text) {
        int start = text.indexOf('{');
        int end = text.lastIndexOf('}');
        if (start < 0 || end <= start) {
            return null;
        }
        return text.substring(start, end + 1);
    }

    private static List<Block> parsePalette(JsonArray paletteArray) {
        List<Block> blocks = new ArrayList<>();
        if (paletteArray == null) {
            return blocks;
        }
        for (JsonElement element : paletteArray) {
            if (!element.isJsonPrimitive()) {
                continue;
            }
            String blockId = element.getAsString();
            if (!ALLOWED_BLOCK_SET.contains(blockId)) {
                continue;
            }
            Block block = BlockMapper.resolvePaletteBlock(blockId);
            if (block != null && !blocks.contains(block)) {
                blocks.add(block);
            }
            if (blocks.size() >= MAX_BLOCKS) {
                break;
            }
        }
        return blocks;
    }

    private static List<Block> resolveBlocks(List<String> ids) {
        List<Block> blocks = new ArrayList<>();
        for (String id : ids) {
            Block block = BlockMapper.resolvePaletteBlock(id);
            if (block != null) {
                blocks.add(block);
            }
        }
        return blocks;
    }

    private static List<String> initializeAllowedPool() {
        List<String> pool = BlockMapper.getPaletteBlockIds();
        if (pool.isEmpty()) {
            return List.of(
                    "minecraft:stone",
                    "minecraft:andesite",
                    "minecraft:oak_planks",
                    "minecraft:white_concrete",
                    "minecraft:black_concrete"
            );
        }
        return pool;
    }

    private static List<Block> deriveFallbackPalette(Voxelizer.VoxelGrid baseGrid) {
        Map<Block, Integer> counts = new HashMap<>();
        for (int rgb : baseGrid.voxels().values()) {
            Block block = BlockMapper.getClosestBlock(rgb).getBlock();
            counts.put(block, counts.getOrDefault(block, 0) + 1);
        }

        List<Map.Entry<Block, Integer>> ranked = new ArrayList<>(counts.entrySet());
        ranked.sort(Comparator.comparingInt((Map.Entry<Block, Integer> e) -> e.getValue()).reversed());

        List<Block> result = new ArrayList<>();
        for (Map.Entry<Block, Integer> entry : ranked) {
            if (!result.contains(entry.getKey())) {
                result.add(entry.getKey());
            }
            if (result.size() >= MAX_BLOCKS) {
                break;
            }
        }

        if (result.isEmpty()) {
            result = resolveBlocks(List.of(
                    "minecraft:stone",
                    "minecraft:andesite",
                    "minecraft:smooth_stone",
                    "minecraft:oak_planks",
                    "minecraft:white_concrete"
            ));
        }
        return result;
    }

    private static Block darkestBlock(List<Block> palette, Block fallback) {
        if (palette == null || palette.isEmpty()) {
            return fallback;
        }
        Block darkest = palette.get(0);
        int min = BlockMapper.getBlockColor(darkest) & 0xFFFFFF;
        double minLuma = luma(min);
        for (Block block : palette) {
            double value = luma(BlockMapper.getBlockColor(block));
            if (value < minLuma) {
                minLuma = value;
                darkest = block;
            }
        }
        return darkest;
    }

    private static Block brightestBlock(List<Block> palette, Block fallback) {
        if (palette == null || palette.isEmpty()) {
            return fallback;
        }
        Block brightest = palette.get(0);
        int max = BlockMapper.getBlockColor(brightest) & 0xFFFFFF;
        double maxLuma = luma(max);
        for (Block block : palette) {
            double value = luma(BlockMapper.getBlockColor(block));
            if (value > maxLuma) {
                maxLuma = value;
                brightest = block;
            }
        }
        return brightest;
    }

    private static Block resolveRole(JsonObject rolesJson, String roleKey, List<Block> palette, int fallbackIndex) {
        if (rolesJson.has(roleKey) && rolesJson.get(roleKey).isJsonPrimitive()) {
            String blockId = rolesJson.get(roleKey).getAsString();
            if (ALLOWED_BLOCK_SET.contains(blockId)) {
                Block block = BlockMapper.resolvePaletteBlock(blockId);
                if (block != null) {
                    return block;
                }
            }
        }
        if (palette.isEmpty()) {
            return null;
        }
        int index = Math.max(0, Math.min(fallbackIndex, palette.size() - 1));
        return palette.get(index);
    }

    private static MaterialRoles fallbackRoles(Voxelizer.VoxelGrid baseGrid) {
        List<Block> fallback = deriveFallbackPalette(baseGrid);
        Block wallPrimary = fallback.size() > 0 ? fallback.get(0) : Blocks.STONE;
        Block wallSecondary = fallback.size() > 1 ? fallback.get(1) : wallPrimary;
        Block foundation = darkestBlock(fallback, wallPrimary);
        Block trim = brightestBlock(fallback, wallPrimary);
        Block accent = fallback.size() > 2 ? fallback.get(2) : null;
        return new MaterialRoles(foundation, wallPrimary, wallSecondary, trim, accent);
    }

    private static StructureStats analyzeStructure(Voxelizer.VoxelGrid grid) {
        Map<BlockPos, Integer> voxels = grid.voxels();
        Set<BlockPos> positions = voxels.keySet();
        int minY = Integer.MAX_VALUE;
        int maxY = Integer.MIN_VALUE;
        int surfaceCount = 0;
        int lowBand = 0;
        int midBand = 0;
        int highBand = 0;

        for (BlockPos pos : positions) {
            minY = Math.min(minY, pos.getY());
            maxY = Math.max(maxY, pos.getY());
        }

        int height = Math.max(1, maxY - minY + 1);
        for (BlockPos pos : positions) {
            if (countExposedFaces(pos, positions) > 0) {
                surfaceCount++;
            }
            double yNorm = (pos.getY() - minY) / (double) height;
            if (yNorm < 0.33) {
                lowBand++;
            } else if (yNorm < 0.66) {
                midBand++;
            } else {
                highBand++;
            }
        }

        String colorSummary = summarizeTopColors(voxels, 8);
        int total = Math.max(1, voxels.size());

        return new StructureStats(
                voxels.size(),
                surfaceCount,
                minY,
                maxY,
                lowBand / (double) total,
                midBand / (double) total,
                highBand / (double) total,
                colorSummary
        );
    }

    private static String summarizeTopColors(Map<BlockPos, Integer> voxels, int maxEntries) {
        Map<Integer, Integer> frequency = new HashMap<>();
        for (int color : voxels.values()) {
            frequency.put(color, frequency.getOrDefault(color, 0) + 1);
        }

        List<Map.Entry<Integer, Integer>> top = new ArrayList<>(frequency.entrySet());
        top.sort(Comparator.comparingInt((Map.Entry<Integer, Integer> e) -> e.getValue()).reversed());
        if (top.size() > maxEntries) {
            top = top.subList(0, maxEntries);
        }

        int total = Math.max(1, voxels.size());
        List<String> parts = new ArrayList<>();
        for (Map.Entry<Integer, Integer> entry : top) {
            String hex = String.format("#%06X", entry.getKey() & 0xFFFFFF);
            double ratio = entry.getValue() / (double) total;
            parts.add(hex + ":" + String.format("%.3f", ratio));
        }
        return String.join(", ", parts);
    }

    private static Map<BlockPos, Integer> assignMaterials(
            Voxelizer.VoxelGrid baseGrid,
            MaterialRoles roles,
            StructureStats stats
    ) {
        Map<BlockPos, Integer> source = baseGrid.voxels();
        Set<BlockPos> allPositions = new HashSet<>(source.keySet());
        Map<BlockPos, Integer> out = new HashMap<>(source.size());
        int height = Math.max(1, stats.maxY - stats.minY + 1);

        for (Map.Entry<BlockPos, Integer> entry : source.entrySet()) {
            BlockPos pos = entry.getKey();
            int rgb = entry.getValue();
            int exposedFaces = countExposedFaces(pos, allPositions);
            boolean isSurface = exposedFaces > 0;
            double yNorm = (pos.getY() - stats.minY) / (double) height;
            double brightness = luma(rgb);

            Block chosen = chooseBlockForVoxel(pos, yNorm, brightness, isSurface, exposedFaces, roles);
            out.put(pos, BlockMapper.getBlockColor(chosen));
        }
        return out;
    }

    private static Block chooseBlockForVoxel(
            BlockPos pos,
            double yNorm,
            double brightness,
            boolean isSurface,
            int exposedFaces,
            MaterialRoles roles
    ) {
        if (yNorm <= 0.17) {
            return roles.foundation();
        }

        if (isSurface && yNorm >= 0.84) {
            return roles.trim();
        }

        if (isSurface && exposedFaces >= 3 && roles.accent() != null && hashedModulo(pos, 37) == 0) {
            return roles.accent();
        }

        if (roles.wallSecondary() != null) {
            boolean lighterSource = brightness > 145;
            boolean occasionalVariation = hashedModulo(pos, 11) == 0;
            if (lighterSource || occasionalVariation) {
                return roles.wallSecondary();
            }
        }

        return roles.wallPrimary();
    }

    private static void smoothSurface(Map<BlockPos, Integer> voxels, int passes) {
        if (voxels.isEmpty()) {
            return;
        }
        Set<BlockPos> positions = voxels.keySet();
        for (int pass = 0; pass < passes; pass++) {
            Map<BlockPos, Integer> updates = new HashMap<>();
            for (BlockPos pos : positions) {
                if (countExposedFaces(pos, positions) == 0) {
                    continue;
                }

                int current = voxels.get(pos);
                Map<Integer, Integer> counts = new HashMap<>();
                for (BlockPos neighbor : cardinalNeighbors(pos)) {
                    Integer color = voxels.get(neighbor);
                    if (color != null) {
                        counts.put(color, counts.getOrDefault(color, 0) + 1);
                    }
                }

                int bestColor = current;
                int bestCount = 0;
                for (Map.Entry<Integer, Integer> count : counts.entrySet()) {
                    if (count.getValue() > bestCount) {
                        bestCount = count.getValue();
                        bestColor = count.getKey();
                    }
                }

                int currentCount = counts.getOrDefault(current, 0);
                if (bestColor != current && bestCount >= 4 && currentCount <= 1) {
                    updates.put(pos, bestColor);
                }
            }
            voxels.putAll(updates);
        }
    }

    private static int countExposedFaces(BlockPos pos, Set<BlockPos> allPositions) {
        int exposed = 0;
        for (BlockPos neighbor : cardinalNeighbors(pos)) {
            if (!allPositions.contains(neighbor)) {
                exposed++;
            }
        }
        return exposed;
    }

    private static List<BlockPos> cardinalNeighbors(BlockPos pos) {
        return List.of(
                pos.above(),
                pos.below(),
                pos.north(),
                pos.south(),
                pos.east(),
                pos.west()
        );
    }

    private static int hashedModulo(BlockPos pos, int modulo) {
        int hash = 17;
        hash = 31 * hash + pos.getX();
        hash = 31 * hash + pos.getY();
        hash = 31 * hash + pos.getZ();
        return Math.floorMod(hash, modulo);
    }

    private static double luma(int rgb) {
        int r = (rgb >> 16) & 0xFF;
        int g = (rgb >> 8) & 0xFF;
        int b = rgb & 0xFF;
        return r * 0.2126 + g * 0.7152 + b * 0.0722;
    }
}
