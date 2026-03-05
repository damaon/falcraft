package com.falcraft.cli;

import com.falcraft.util.GLBParser;
import com.falcraft.util.TextureSampler;
import com.falcraft.util.Voxelizer;
import net.minecraft.core.BlockPos;

import javax.swing.JFrame;
import javax.swing.JPanel;
import javax.swing.SwingUtilities;
import javax.swing.Timer;
import java.awt.BasicStroke;
import java.awt.Color;
import java.awt.Font;
import java.awt.GradientPaint;
import java.awt.Graphics;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.event.KeyAdapter;
import java.awt.event.KeyEvent;
import java.awt.event.MouseAdapter;
import java.awt.event.MouseEvent;
import java.awt.event.MouseWheelEvent;
import java.awt.geom.Rectangle2D;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

/**
 * Desktop viewer window for standalone GLB preview.
 * Mesh mode renders triangles.
 * Voxel mode renders point-sprite cubes from Voxelizer output.
 */
public class GlbViewerFrame extends JFrame {
    private static final int WINDOW_WIDTH = 1200;
    private static final int WINDOW_HEIGHT = 820;
    private static final int TARGET_FPS = 30;

    private static final int MAX_RENDER_TRIANGLES = 25_000;
    private static final int MAX_RENDER_VOXELS = 120_000;
    private static final int CLI_PROGRESS_STEP_PERCENT = 5;
    private static final ExecutorService VOXEL_EXECUTOR = Executors.newSingleThreadExecutor(new ThreadFactory() {
        @Override
        public Thread newThread(Runnable r) {
            Thread thread = new Thread(r, "falcraft-glb-voxelizer");
            thread.setDaemon(true);
            return thread;
        }
    });

    private final String sourceName;
    private final int gridSize;
    private final GLBParser.MeshData meshData;
    private final TextureSampler textureSampler;
    private final float[] normalizedVertices;
    private final int[] indices;
    private final int[] colors;
    private final int triangleStride;

    private final JPanel panel;
    private final Timer repaintTimer;

    private List<VoxelPoint> voxelPoints;
    private volatile CompletableFuture<Voxelizer.VoxelGrid> voxelFuture;
    private volatile String voxelStatus = "Idle (press V to start voxelization)";
    private volatile int voxelProgressPercent = 0;
    private volatile int lastLoggedProgressPercent = -1;
    private int fullVoxelCount = 0;

    private boolean showVoxelized = false;
    private float yaw = (float) Math.toRadians(26);
    private float pitch = (float) Math.toRadians(-17);
    private float zoom = 3.4f;
    private int lastMouseX = 0;
    private int lastMouseY = 0;
    private boolean dragging = false;

    public GlbViewerFrame(
            java.nio.file.Path sourcePath,
            GLBParser.MeshData meshData,
            TextureSampler textureSampler,
            int gridSize
    ) {
        this.sourceName = sourcePath.getFileName() != null ? sourcePath.getFileName().toString() : sourcePath.toString();
        this.gridSize = gridSize;
        this.meshData = meshData;
        this.textureSampler = textureSampler;
        this.indices = meshData.indices();
        this.colors = meshData.colors();
        this.normalizedVertices = normalizeVertices(meshData.vertices());
        this.triangleStride = computeTriangleStride(this.indices.length / 3);

        setTitle("Falcraft GLB Viewer");
        setSize(WINDOW_WIDTH, WINDOW_HEIGHT);
        setMinimumSize(new java.awt.Dimension(800, 560));
        setLocationRelativeTo(null);
        setDefaultCloseOperation(DISPOSE_ON_CLOSE);

        this.panel = createPanel();
        setContentPane(panel);
        bindInput(panel);

        int delayMs = Math.max(1, 1000 / TARGET_FPS);
        this.repaintTimer = new Timer(delayMs, e -> {
            pollVoxelFuture();
            panel.repaint();
        });
        this.repaintTimer.start();

        addWindowListener(new java.awt.event.WindowAdapter() {
            @Override
            public void windowClosed(java.awt.event.WindowEvent e) {
                repaintTimer.stop();
            }
        });

        SwingUtilities.invokeLater(panel::requestFocusInWindow);
    }

    private JPanel createPanel() {
        return new JPanel() {
            @Override
            protected void paintComponent(Graphics g) {
                super.paintComponent(g);
                Graphics2D g2 = (Graphics2D) g.create();
                try {
                    drawScene(g2, getWidth(), getHeight());
                } finally {
                    g2.dispose();
                }
            }
        };
    }

    private void bindInput(JPanel target) {
        target.setFocusable(true);

        target.addMouseListener(new MouseAdapter() {
            @Override
            public void mousePressed(MouseEvent e) {
                dragging = true;
                lastMouseX = e.getX();
                lastMouseY = e.getY();
            }

            @Override
            public void mouseReleased(MouseEvent e) {
                dragging = false;
            }
        });

        target.addMouseMotionListener(new MouseAdapter() {
            @Override
            public void mouseDragged(MouseEvent e) {
                if (!dragging) {
                    return;
                }
                int dx = e.getX() - lastMouseX;
                int dy = e.getY() - lastMouseY;
                lastMouseX = e.getX();
                lastMouseY = e.getY();

                yaw += dx * 0.01f;
                pitch = clamp(pitch + dy * 0.01f, (float) -Math.PI * 0.45f, (float) Math.PI * 0.45f);
                target.repaint();
            }
        });

        target.addMouseWheelListener((MouseWheelEvent e) -> {
            zoom = clamp(zoom + e.getWheelRotation() * 0.18f, 1.4f, 12.0f);
            target.repaint();
        });

        target.addKeyListener(new KeyAdapter() {
            @Override
            public void keyPressed(KeyEvent e) {
                switch (e.getKeyCode()) {
                    case KeyEvent.VK_V -> {
                        showVoxelized = !showVoxelized;
                        if (showVoxelized && voxelPoints == null) {
                            startVoxelizationIfNeeded();
                        }
                    }
                    case KeyEvent.VK_R -> resetCamera();
                    case KeyEvent.VK_Q, KeyEvent.VK_ESCAPE -> dispose();
                    default -> {
                        return;
                    }
                }
                target.repaint();
            }
        });
    }

    private void drawScene(Graphics2D g2, int width, int height) {
        g2.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_OFF);
        g2.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_SPEED);

        GradientPaint bg = new GradientPaint(
                0, 0, new Color(11, 14, 22),
                width, height, new Color(20, 24, 35)
        );
        g2.setPaint(bg);
        g2.fillRect(0, 0, width, height);

        if (showVoxelized) {
            if (voxelPoints != null) {
                drawVoxelView(g2, width, height);
            } else {
                drawMeshView(g2, width, height);
            }
        } else {
            drawMeshView(g2, width, height);
        }

        drawHud(g2, width, height);
    }

    private void drawMeshView(Graphics2D g2, int width, int height) {
        if (indices.length < 3) {
            return;
        }

        int vertexCount = normalizedVertices.length / 3;
        ProjectedPoint[] projected = new ProjectedPoint[vertexCount];
        for (int i = 0; i < vertexCount; i++) {
            float x = normalizedVertices[i * 3];
            float y = normalizedVertices[i * 3 + 1];
            float z = normalizedVertices[i * 3 + 2];
            projected[i] = project(x, y, z, width, height);
        }

        List<RenderTriangle> triangles = new ArrayList<>(indices.length / (3 * triangleStride));
        int totalTriangles = indices.length / 3;
        for (int tri = 0; tri < totalTriangles; tri += triangleStride) {
            int base = tri * 3;
            int i0 = indices[base];
            int i1 = indices[base + 1];
            int i2 = indices[base + 2];

            if (!isValidVertexIndex(i0, projected) || !isValidVertexIndex(i1, projected) || !isValidVertexIndex(i2, projected)) {
                continue;
            }

            ProjectedPoint p0 = projected[i0];
            ProjectedPoint p1 = projected[i1];
            ProjectedPoint p2 = projected[i2];
            if (!p0.visible || !p1.visible || !p2.visible) {
                continue;
            }

            int rgb = averageColor(i0, i1, i2);
            float depth = (p0.depth + p1.depth + p2.depth) / 3f;
            triangles.add(new RenderTriangle(
                    new int[] {p0.sx, p1.sx, p2.sx},
                    new int[] {p0.sy, p1.sy, p2.sy},
                    depth,
                    rgb
            ));
        }

        triangles.sort(Comparator.comparingDouble(t -> -t.depth));

        g2.setStroke(new BasicStroke(1.0f));
        for (RenderTriangle triangle : triangles) {
            int r = (triangle.rgb >> 16) & 0xFF;
            int g = (triangle.rgb >> 8) & 0xFF;
            int b = triangle.rgb & 0xFF;

            g2.setColor(new Color(r, g, b, 170));
            g2.fillPolygon(triangle.xs, triangle.ys, 3);

            g2.setColor(new Color(12, 14, 18, 140));
            g2.drawPolygon(triangle.xs, triangle.ys, 3);
        }
    }

    private void drawVoxelView(Graphics2D g2, int width, int height) {
        if (voxelPoints == null || voxelPoints.isEmpty()) {
            return;
        }

        List<RenderVoxel> renderPoints = new ArrayList<>(voxelPoints.size());
        for (VoxelPoint voxel : voxelPoints) {
            ProjectedPoint projected = project(voxel.x, voxel.y, voxel.z, width, height);
            if (!projected.visible) {
                continue;
            }
            renderPoints.add(new RenderVoxel(projected.sx, projected.sy, projected.depth, voxel.rgb));
        }

        renderPoints.sort(Comparator.comparingDouble(v -> -v.depth));
        for (RenderVoxel voxel : renderPoints) {
            int r = (voxel.rgb >> 16) & 0xFF;
            int g = (voxel.rgb >> 8) & 0xFF;
            int b = voxel.rgb & 0xFF;
            int size = Math.max(2, Math.min(22, Math.round(14f / voxel.depth)));

            g2.setColor(new Color(r, g, b, 210));
            g2.fillRect(voxel.sx - size / 2, voxel.sy - size / 2, size, size);
            g2.setColor(new Color(8, 10, 12, 120));
            g2.drawRect(voxel.sx - size / 2, voxel.sy - size / 2, size, size);
        }
    }

    private void drawHud(Graphics2D g2, int width, int height) {
        final int padding = 14;
        final int lineHeight = 18;
        int y = padding + lineHeight;

        g2.setFont(new Font("Monospaced", Font.PLAIN, 13));

        String modeLabel = showVoxelized ? "VOXELIZED" : "MESH";
        String voxelLabel;
        if (voxelPoints != null) {
            voxelLabel = "voxels: " + fullVoxelCount + (voxelPoints.size() < fullVoxelCount ? " (sampled " + voxelPoints.size() + ")" : "");
        } else {
            voxelLabel = voxelStatus;
        }

        List<String> lines = List.of(
                "File: " + sourceName,
                "Mode: " + modeLabel + " | Grid: " + gridSize,
                "Mesh triangles: " + (indices.length / 3) + (triangleStride > 1 ? " (render step " + triangleStride + ")" : ""),
                "Voxel status: " + voxelLabel,
                "Controls: drag orbit | wheel zoom | V toggle mesh/voxel | R reset | Esc/Q close"
        );

        int maxWidth = 0;
        for (String line : lines) {
            Rectangle2D bounds = g2.getFontMetrics().getStringBounds(line, g2);
            maxWidth = Math.max(maxWidth, (int) Math.ceil(bounds.getWidth()));
        }

        int boxWidth = maxWidth + padding * 2;
        int boxHeight = lines.size() * lineHeight + padding;

        g2.setColor(new Color(0, 0, 0, 120));
        g2.fillRoundRect(padding - 6, padding - 4, boxWidth, boxHeight, 12, 12);

        g2.setColor(new Color(235, 240, 248));
        for (String line : lines) {
            g2.drawString(line, padding, y);
            y += lineHeight;
        }

        if (showVoxelized && voxelPoints == null) {
            g2.setColor(new Color(255, 210, 120));
            g2.drawString("Voxel mode requested; waiting for voxelization to finish...", padding, height - 20);
        }
    }

    private void pollVoxelFuture() {
        if (voxelPoints != null || !voxelFuture.isDone()) {
            return;
        }
        try {
            Voxelizer.VoxelGrid grid = voxelFuture.get();
            fullVoxelCount = grid.voxels().size();
            voxelPoints = buildVoxelPoints(grid);
            voxelStatus = "Ready";
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            voxelStatus = "Voxelization interrupted";
        } catch (CompletionException e) {
            voxelStatus = "Voxelization failed: " + cleanErrorMessage(e.getCause());
        } catch (Exception e) {
            voxelStatus = "Voxelization failed: " + cleanErrorMessage(e);
        }
    }

    private List<VoxelPoint> buildVoxelPoints(Voxelizer.VoxelGrid grid) {
        Map<BlockPos, Integer> voxels = grid.voxels();
        int size = Math.max(1, grid.size() - 1);
        List<VoxelPoint> all = new ArrayList<>(voxels.size());

        for (Map.Entry<BlockPos, Integer> entry : voxels.entrySet()) {
            BlockPos pos = entry.getKey();
            float x = size == 0 ? 0f : ((float) pos.getX() / size - 0.5f) * 2.0f;
            float y = size == 0 ? 0f : ((float) pos.getY() / size - 0.5f) * 2.0f;
            float z = size == 0 ? 0f : ((float) pos.getZ() / size - 0.5f) * 2.0f;
            all.add(new VoxelPoint(x, y, z, entry.getValue()));
        }

        if (all.size() <= MAX_RENDER_VOXELS) {
            return all;
        }

        int step = (int) Math.ceil((double) all.size() / MAX_RENDER_VOXELS);
        List<VoxelPoint> sampled = new ArrayList<>(MAX_RENDER_VOXELS);
        for (int i = 0; i < all.size(); i += step) {
            sampled.add(all.get(i));
        }
        return sampled;
    }

    private ProjectedPoint project(float x, float y, float z, int width, int height) {
        float cosYaw = (float) Math.cos(yaw);
        float sinYaw = (float) Math.sin(yaw);
        float cosPitch = (float) Math.cos(pitch);
        float sinPitch = (float) Math.sin(pitch);

        float x1 = cosYaw * x + sinYaw * z;
        float z1 = -sinYaw * x + cosYaw * z;

        float y2 = cosPitch * y - sinPitch * z1;
        float z2 = sinPitch * y + cosPitch * z1;

        float depth = z2 + zoom;
        if (depth <= 0.06f) {
            return ProjectedPoint.INVISIBLE;
        }

        float focal = Math.min(width, height) * 0.75f;
        int sx = Math.round(width * 0.5f + (x1 * focal / depth));
        int sy = Math.round(height * 0.5f - (y2 * focal / depth));
        return new ProjectedPoint(sx, sy, depth, true);
    }

    private int averageColor(int i0, int i1, int i2) {
        if (!isValidColorIndex(i0) || !isValidColorIndex(i1) || !isValidColorIndex(i2)) {
            return 0xB8BEC8;
        }
        int c0 = colors[i0];
        int c1 = colors[i1];
        int c2 = colors[i2];
        int r = (((c0 >> 16) & 0xFF) + ((c1 >> 16) & 0xFF) + ((c2 >> 16) & 0xFF)) / 3;
        int g = (((c0 >> 8) & 0xFF) + ((c1 >> 8) & 0xFF) + ((c2 >> 8) & 0xFF)) / 3;
        int b = ((c0 & 0xFF) + (c1 & 0xFF) + (c2 & 0xFF)) / 3;
        return (r << 16) | (g << 8) | b;
    }

    private static float[] normalizeVertices(float[] vertices) {
        if (vertices.length == 0) {
            return vertices;
        }

        float minX = Float.MAX_VALUE;
        float minY = Float.MAX_VALUE;
        float minZ = Float.MAX_VALUE;
        float maxX = -Float.MAX_VALUE;
        float maxY = -Float.MAX_VALUE;
        float maxZ = -Float.MAX_VALUE;

        for (int i = 0; i < vertices.length; i += 3) {
            float x = vertices[i];
            float y = vertices[i + 1];
            float z = vertices[i + 2];
            minX = Math.min(minX, x);
            minY = Math.min(minY, y);
            minZ = Math.min(minZ, z);
            maxX = Math.max(maxX, x);
            maxY = Math.max(maxY, y);
            maxZ = Math.max(maxZ, z);
        }

        float cx = (minX + maxX) * 0.5f;
        float cy = (minY + maxY) * 0.5f;
        float cz = (minZ + maxZ) * 0.5f;

        float dx = maxX - minX;
        float dy = maxY - minY;
        float dz = maxZ - minZ;
        float longest = Math.max(dx, Math.max(dy, dz));
        if (longest < 1.0e-6f) {
            longest = 1.0f;
        }
        float scale = 1.9f / longest;

        float[] normalized = new float[vertices.length];
        for (int i = 0; i < vertices.length; i += 3) {
            normalized[i] = (vertices[i] - cx) * scale;
            normalized[i + 1] = (vertices[i + 1] - cy) * scale;
            normalized[i + 2] = (vertices[i + 2] - cz) * scale;
        }
        return normalized;
    }

    private static int computeTriangleStride(int triangleCount) {
        if (triangleCount <= MAX_RENDER_TRIANGLES) {
            return 1;
        }
        return (int) Math.ceil((double) triangleCount / MAX_RENDER_TRIANGLES);
    }

    private static float clamp(float value, float min, float max) {
        return Math.max(min, Math.min(max, value));
    }

    private void resetCamera() {
        yaw = (float) Math.toRadians(26);
        pitch = (float) Math.toRadians(-17);
        zoom = 3.4f;
    }

    private boolean isValidColorIndex(int idx) {
        return idx >= 0 && idx < colors.length;
    }

    private static boolean isValidVertexIndex(int idx, ProjectedPoint[] vertices) {
        return idx >= 0 && idx < vertices.length;
    }

    private static String cleanErrorMessage(Throwable error) {
        if (error == null || error.getMessage() == null || error.getMessage().isBlank()) {
            return "unknown error";
        }
        return error.getMessage().replace('\n', ' ').trim();
    }

    private record ProjectedPoint(int sx, int sy, float depth, boolean visible) {
        private static final ProjectedPoint INVISIBLE = new ProjectedPoint(0, 0, 0f, false);
    }

    private record RenderTriangle(int[] xs, int[] ys, float depth, int rgb) {
    }

    private record VoxelPoint(float x, float y, float z, int rgb) {
    }

    private record RenderVoxel(int sx, int sy, float depth, int rgb) {
    }
}
