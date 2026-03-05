package com.falcraft.cli;

import com.falcraft.util.GLBParser;
import com.falcraft.util.TextureSampler;
import com.falcraft.util.Voxelizer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import javax.swing.SwingUtilities;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.CompletableFuture;

/**
 * Standalone CLI GLB viewer.
 * Usage:
 *   ./glb-viewer /path/to/model.glb --grid 64
 *   ./gradlew runGlbViewer --args="/path/to/model.glb --grid 64"
 */
public final class GlbViewerCli {
    private static final Logger LOGGER = LoggerFactory.getLogger("GlbViewerCli");
    private static final int DEFAULT_GRID_SIZE = 64;
    private static final int DEFAULT_TARGET_TRIANGLES = 60_000;

    private GlbViewerCli() {
    }

    public static void main(String[] args) {
        try {
            CliOptions options = parseArgs(args);
            byte[] glbData = Files.readAllBytes(options.path());

            TextureSampler textureSampler = null;
            try {
                byte[] embeddedTexture = GLBParser.extractEmbeddedTexture(glbData);
                if (embeddedTexture != null) {
                    textureSampler = new TextureSampler(embeddedTexture);
                    LOGGER.info("Embedded texture loaded for voxel sampling");
                } else {
                    LOGGER.info("No embedded texture found, using vertex/fallback colors");
                }
            } catch (Exception e) {
                LOGGER.warn("Failed to load embedded texture: {}", e.getMessage());
            }

            GLBParser.MeshData meshData = GLBParser.parse(glbData, textureSampler);
            if (meshData.vertices().length == 0 || meshData.indices().length == 0) {
                throw new IllegalArgumentException("Model contains no renderable geometry");
            }

            GLBParser.MeshData workingMesh = meshData;
            int sourceTriangles = meshData.indices().length / 3;
            if (options.autoSimplify() && sourceTriangles > options.targetTriangles()) {
                LOGGER.info("Simplifying mesh from {} to ~{} triangles", sourceTriangles, options.targetTriangles());
                workingMesh = MeshSimplifier.simplify(meshData, options.targetTriangles());
                LOGGER.info("Simplified mesh triangle count: {}", workingMesh.indices().length / 3);
            } else {
                LOGGER.info("Using original mesh triangle count: {}", sourceTriangles);
            }
            final GLBParser.MeshData finalMesh = workingMesh;

            final TextureSampler samplerForVoxelization = textureSampler;
            CompletableFuture<Voxelizer.VoxelGrid> voxelFuture = CompletableFuture.supplyAsync(
                    () -> Voxelizer.voxelize(finalMesh, options.gridSize(), samplerForVoxelization)
            );

            SwingUtilities.invokeLater(() -> {
                GlbViewerFrame frame = new GlbViewerFrame(options.path(), finalMesh, voxelFuture, options.gridSize());
                frame.setVisible(true);
            });
        } catch (IllegalArgumentException e) {
            System.err.println("Error: " + e.getMessage());
            printUsage();
            System.exit(1);
        } catch (IOException e) {
            System.err.println("Error: " + e.getMessage());
            System.exit(1);
        } catch (Exception e) {
            System.err.println("Unexpected error: " + e.getMessage());
            e.printStackTrace(System.err);
            System.exit(1);
        }
    }

    private static CliOptions parseArgs(String[] args) {
        if (args.length == 0) {
            throw new IllegalArgumentException("Missing GLB path");
        }

        Path glbPath = null;
        int gridSize = DEFAULT_GRID_SIZE;
        int targetTriangles = DEFAULT_TARGET_TRIANGLES;
        boolean autoSimplify = true;

        for (int i = 0; i < args.length; i++) {
            String arg = args[i];
            switch (arg) {
                case "-h", "--help" -> {
                    printUsage();
                    System.exit(0);
                }
                case "-g", "--grid" -> {
                    if (i + 1 >= args.length) {
                        throw new IllegalArgumentException("Missing value for " + arg);
                    }
                    i++;
                    try {
                        gridSize = Integer.parseInt(args[i]);
                    } catch (NumberFormatException e) {
                        throw new IllegalArgumentException("Grid size must be an integer: " + args[i]);
                    }
                }
                case "--target-tris" -> {
                    if (i + 1 >= args.length) {
                        throw new IllegalArgumentException("Missing value for " + arg);
                    }
                    i++;
                    try {
                        targetTriangles = Integer.parseInt(args[i]);
                    } catch (NumberFormatException e) {
                        throw new IllegalArgumentException("Target triangles must be an integer: " + args[i]);
                    }
                }
                case "--no-lowpoly" -> autoSimplify = false;
                default -> {
                    if (arg.startsWith("-")) {
                        throw new IllegalArgumentException("Unknown option: " + arg);
                    }
                    if (glbPath != null) {
                        throw new IllegalArgumentException("Only one GLB path is supported");
                    }
                    glbPath = Path.of(arg);
                }
            }
        }

        if (glbPath == null) {
            throw new IllegalArgumentException("Missing GLB path");
        }
        if (!Files.exists(glbPath)) {
            throw new IllegalArgumentException("GLB file does not exist: " + glbPath);
        }
        if (!Files.isRegularFile(glbPath)) {
            throw new IllegalArgumentException("Not a regular file: " + glbPath);
        }
        if (gridSize < 8 || gridSize > 256) {
            throw new IllegalArgumentException("Grid size must be between 8 and 256");
        }
        if (targetTriangles < 1_000 || targetTriangles > 500_000) {
            throw new IllegalArgumentException("Target triangles must be between 1000 and 500000");
        }

        return new CliOptions(glbPath.toAbsolutePath().normalize(), gridSize, targetTriangles, autoSimplify);
    }

    private static void printUsage() {
        System.out.println("Standalone GLB viewer");
        System.out.println();
        System.out.println("Usage:");
        System.out.println("  ./glb-viewer <path-to-model.glb> [--grid 64] [--target-tris 60000]");
        System.out.println("  ./gradlew runGlbViewer --args=\"<path-to-model.glb> [--grid 64] [--target-tris 60000]\"");
        System.out.println();
        System.out.println("Options:");
        System.out.println("  -g, --grid <n>   Voxelization grid size (8-256, default: 64)");
        System.out.println("  --target-tris    Simplify mesh target (1000-500000, default: 60000)");
        System.out.println("  --no-lowpoly     Disable simplification (uses full mesh)");
        System.out.println("  -h, --help       Show this help");
        System.out.println();
        System.out.println("Controls:");
        System.out.println("  Drag mouse       Orbit camera");
        System.out.println("  Mouse wheel      Zoom");
        System.out.println("  V                Toggle mesh <-> voxelized");
        System.out.println("  R                Reset camera");
        System.out.println("  Esc / Q          Close viewer");
    }

    private record CliOptions(Path path, int gridSize, int targetTriangles, boolean autoSimplify) {
    }
}
