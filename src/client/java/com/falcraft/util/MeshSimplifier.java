package com.falcraft.util;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Lightweight mesh simplifier for viewer/voxelization performance.
 * Uses vertex clustering, then applies triangle sampling if still over target.
 */
public final class MeshSimplifier {
    private static final int MIN_GRID = 8;
    private static final int MAX_GRID = 256;

    private MeshSimplifier() {
    }

    public static GLBParser.MeshData simplify(GLBParser.MeshData input, int targetTriangles) {
        int sourceTriangles = input.indices().length / 3;
        if (sourceTriangles <= targetTriangles || targetTriangles <= 0) {
            return input;
        }

        SimplifyResult clustered = clusterVertices(input, targetTriangles);
        int clusteredTriangles = clustered.indices().size() / 3;

        if (clusteredTriangles > targetTriangles) {
            sampledTriangleReduction(clustered.indices(), targetTriangles);
        }

        return toMeshData(clustered);
    }

    private static SimplifyResult clusterVertices(GLBParser.MeshData input, int targetTriangles) {
        float[] vertices = input.vertices();
        int[] indices = input.indices();
        int[] colors = input.colors();
        float[] uvs = input.uvs();

        int vertexCount = vertices.length / 3;
        boolean hasColors = colors != null && colors.length >= vertexCount;
        boolean hasUVs = uvs != null && uvs.length >= vertexCount * 2;

        float minX = Float.MAX_VALUE;
        float minY = Float.MAX_VALUE;
        float minZ = Float.MAX_VALUE;
        float maxX = -Float.MAX_VALUE;
        float maxY = -Float.MAX_VALUE;
        float maxZ = -Float.MAX_VALUE;

        for (int i = 0; i < vertexCount; i++) {
            float x = vertices[i * 3];
            float y = vertices[i * 3 + 1];
            float z = vertices[i * 3 + 2];
            minX = Math.min(minX, x);
            minY = Math.min(minY, y);
            minZ = Math.min(minZ, z);
            maxX = Math.max(maxX, x);
            maxY = Math.max(maxY, y);
            maxZ = Math.max(maxZ, z);
        }

        float dx = Math.max(1e-6f, maxX - minX);
        float dy = Math.max(1e-6f, maxY - minY);
        float dz = Math.max(1e-6f, maxZ - minZ);

        int originalTriangles = indices.length / 3;
        double ratio = Math.max(1e-5, Math.min(1.0, (double) targetTriangles / originalTriangles));
        int targetVertices = Math.max(64, (int) Math.round(vertexCount * ratio));
        int grid = (int) Math.round(Math.cbrt(targetVertices));
        grid = Math.max(MIN_GRID, Math.min(MAX_GRID, grid));

        Map<Long, Cluster> clustersByCell = new HashMap<>();
        int[] remap = new int[vertexCount];

        for (int i = 0; i < vertexCount; i++) {
            float x = vertices[i * 3];
            float y = vertices[i * 3 + 1];
            float z = vertices[i * 3 + 2];

            int gx = quantize(x, minX, dx, grid);
            int gy = quantize(y, minY, dy, grid);
            int gz = quantize(z, minZ, dz, grid);
            long key = packCell(gx, gy, gz);

            Cluster cluster = clustersByCell.get(key);
            if (cluster == null) {
                cluster = new Cluster();
                cluster.newIndex = clustersByCell.size();
                clustersByCell.put(key, cluster);
            }

            cluster.count++;
            cluster.sumX += x;
            cluster.sumY += y;
            cluster.sumZ += z;

            if (hasColors) {
                int color = colors[i];
                cluster.sumR += (color >> 16) & 0xFF;
                cluster.sumG += (color >> 8) & 0xFF;
                cluster.sumB += color & 0xFF;
            }

            if (hasUVs) {
                cluster.sumU += uvs[i * 2];
                cluster.sumV += uvs[i * 2 + 1];
            }

            remap[i] = cluster.newIndex;
        }

        List<Cluster> orderedClusters = new ArrayList<>(clustersByCell.values());
        orderedClusters.sort((a, b) -> Integer.compare(a.newIndex, b.newIndex));

        List<Integer> outIndices = new ArrayList<>(indices.length);
        for (int i = 0; i < indices.length; i += 3) {
            int a = remap[indices[i]];
            int b = remap[indices[i + 1]];
            int c = remap[indices[i + 2]];
            if (a == b || b == c || a == c) {
                continue;
            }
            outIndices.add(a);
            outIndices.add(b);
            outIndices.add(c);
        }

        return new SimplifyResult(orderedClusters, outIndices, hasColors, hasUVs);
    }

    private static void sampledTriangleReduction(List<Integer> indices, int targetTriangles) {
        int currentTriangles = indices.size() / 3;
        if (currentTriangles <= targetTriangles) {
            return;
        }

        int step = (int) Math.ceil((double) currentTriangles / targetTriangles);
        List<Integer> sampled = new ArrayList<>(targetTriangles * 3);
        for (int tri = 0; tri < currentTriangles; tri += step) {
            int base = tri * 3;
            sampled.add(indices.get(base));
            sampled.add(indices.get(base + 1));
            sampled.add(indices.get(base + 2));
        }

        indices.clear();
        indices.addAll(sampled);
    }

    private static GLBParser.MeshData toMeshData(SimplifyResult result) {
        int vertexCount = result.clusters().size();
        float[] vertices = new float[vertexCount * 3];
        int[] colors = new int[vertexCount];
        float[] uvs = new float[vertexCount * 2];

        for (int i = 0; i < vertexCount; i++) {
            Cluster c = result.clusters().get(i);
            float inv = 1.0f / c.count;
            vertices[i * 3] = c.sumX * inv;
            vertices[i * 3 + 1] = c.sumY * inv;
            vertices[i * 3 + 2] = c.sumZ * inv;

            if (result.hasColors()) {
                int r = clamp255(Math.round(c.sumR * inv));
                int g = clamp255(Math.round(c.sumG * inv));
                int b = clamp255(Math.round(c.sumB * inv));
                colors[i] = (r << 16) | (g << 8) | b;
            } else {
                colors[i] = 0x808080;
            }

            if (result.hasUVs()) {
                uvs[i * 2] = c.sumU * inv;
                uvs[i * 2 + 1] = c.sumV * inv;
            } else {
                uvs[i * 2] = 0f;
                uvs[i * 2 + 1] = 0f;
            }
        }

        int[] remappedIndices = new int[result.indices().size()];
        for (int i = 0; i < result.indices().size(); i++) {
            remappedIndices[i] = result.indices().get(i);
        }

        return new GLBParser.MeshData(vertices, remappedIndices, colors, uvs);
    }

    private static int quantize(float value, float min, float range, int grid) {
        float normalized = (value - min) / range;
        int bin = (int) Math.floor(normalized * grid);
        if (bin < 0) {
            return 0;
        }
        if (bin >= grid) {
            return grid - 1;
        }
        return bin;
    }

    private static long packCell(int x, int y, int z) {
        return ((long) x << 42) | ((long) y << 21) | (z & 0x1FFFFFL);
    }

    private static int clamp255(int value) {
        if (value < 0) {
            return 0;
        }
        if (value > 255) {
            return 255;
        }
        return value;
    }

    private static class Cluster {
        int newIndex;
        int count;
        float sumX;
        float sumY;
        float sumZ;
        float sumU;
        float sumV;
        float sumR;
        float sumG;
        float sumB;
    }

    private record SimplifyResult(List<Cluster> clusters, List<Integer> indices, boolean hasColors, boolean hasUVs) {
    }
}
