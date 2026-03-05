package com.falcraft.util;

import net.minecraft.core.BlockPos;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Converts 3D mesh data to a voxel grid using the obj2voxel algorithm.
 * 
 * This implementation uses TRIANGLE SPLITTING instead of polygon clipping.
 * For each voxel, we split the triangle against all 6 axis-aligned planes
 * of the voxel cube, keeping only the portions that fall inside.
 * 
 * Reference: https://github.com/eisenwave/obj2voxel
 */
public class Voxelizer {
    private static final Logger LOGGER = LoggerFactory.getLogger("Voxelizer");
    
    // Epsilon for floating point comparisons
    private static final double EPSILON = 1.0 / (1 << 16);
    
    // Distance limit for plane distance test optimization
    private static final double DISTANCE_LIMIT = 2.0; // sqrt(3) ≈ 1.73 with some leeway
    
    // Saturation boost factor for SAM-3D vertex colors (they tend to be desaturated)
    // 1.4 = 40% boost (moderate - avoids hue shifts on yellows)
    private static final float SATURATION_BOOST = 1.0f;
    
    /**
     * Represents a voxelized 3D model
     */
    public record VoxelGrid(Map<BlockPos, Integer> voxels, int size) {}

    /**
     * Progress callback for long-running voxelization jobs.
     */
    @FunctionalInterface
    public interface ProgressListener {
        void onProgress(int trianglesProcessed, int totalTriangles, int voxelCandidates);
    }
    
    /**
     * 2D vector for UV coordinates
     */
    private static class Vec2 {
        double u, v;
        
        Vec2(double u, double v) {
            this.u = u;
            this.v = v;
        }
        
        static Vec2 mix(Vec2 a, Vec2 b, double t) {
            return new Vec2(
                a.u + (b.u - a.u) * t,
                a.v + (b.v - a.v) * t
            );
        }
        
        static Vec2 add(Vec2 a, Vec2 b) {
            return new Vec2(a.u + b.u, a.v + b.v);
        }
        
        static Vec2 scale(Vec2 v, double s) {
            return new Vec2(v.u * s, v.v * s);
        }
    }
    
    /**
     * 3D vector
     */
    private static class Vec3 {
        double x, y, z;
        
        Vec3(double x, double y, double z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }
        
        double get(int axis) {
            return switch (axis) {
                case 0 -> x;
                case 1 -> y;
                case 2 -> z;
                default -> throw new IllegalArgumentException("Invalid axis: " + axis);
            };
        }
        
        static Vec3 sub(Vec3 a, Vec3 b) {
            return new Vec3(a.x - b.x, a.y - b.y, a.z - b.z);
        }
        
        static Vec3 add(Vec3 a, Vec3 b) {
            return new Vec3(a.x + b.x, a.y + b.y, a.z + b.z);
        }
        
        static Vec3 mix(Vec3 a, Vec3 b, double t) {
            return new Vec3(
                a.x + (b.x - a.x) * t,
                a.y + (b.y - a.y) * t,
                a.z + (b.z - a.z) * t
            );
        }
        
        static Vec3 cross(Vec3 a, Vec3 b) {
            return new Vec3(
                a.y * b.z - a.z * b.y,
                a.z * b.x - a.x * b.z,
                a.x * b.y - a.y * b.x
            );
        }
        
        static double dot(Vec3 a, Vec3 b) {
            return a.x * b.x + a.y * b.y + a.z * b.z;
        }
        
        double length() {
            return Math.sqrt(x * x + y * y + z * z);
        }
        
        Vec3 normalize() {
            double len = length();
            if (len > 0) {
                return new Vec3(x / len, y / len, z / len);
            }
            return new Vec3(0, 0, 0);
        }
    }
    
    /**
     * A textured triangle with vertices, UV coordinates, and vertex colors
     */
    private static class TexturedTriangle {
        Vec3[] v = new Vec3[3];  // Vertices
        Vec2[] t = new Vec2[3];  // Texture coordinates
        int[] c = new int[3];    // Vertex colors (RGB packed)
        
        TexturedTriangle(Vec3 v0, Vec3 v1, Vec3 v2, Vec2 t0, Vec2 t1, Vec2 t2) {
            v[0] = v0; v[1] = v1; v[2] = v2;
            t[0] = t0; t[1] = t1; t[2] = t2;
            c[0] = 0x808080; c[1] = 0x808080; c[2] = 0x808080; // Default gray
        }
        
        TexturedTriangle(Vec3 v0, Vec3 v1, Vec3 v2, Vec2 t0, Vec2 t1, Vec2 t2, int c0, int c1, int c2) {
            v[0] = v0; v[1] = v1; v[2] = v2;
            t[0] = t0; t[1] = t1; t[2] = t2;
            c[0] = c0; c[1] = c1; c[2] = c2;
        }
        
        TexturedTriangle copy() {
            return new TexturedTriangle(
                new Vec3(v[0].x, v[0].y, v[0].z),
                new Vec3(v[1].x, v[1].y, v[1].z),
                new Vec3(v[2].x, v[2].y, v[2].z),
                new Vec2(t[0].u, t[0].v),
                new Vec2(t[1].u, t[1].v),
                new Vec2(t[2].u, t[2].v),
                c[0], c[1], c[2]
            );
        }
        
        Vec3 vertex(int i) { return v[i]; }
        Vec2 texture(int i) { return t[i]; }
        int color(int i) { return c[i]; }
        
        /** Returns the (unnormalized) normal */
        Vec3 normal() {
            return Vec3.cross(Vec3.sub(v[1], v[0]), Vec3.sub(v[2], v[0]));
        }
        
        /** Returns the area of the triangle */
        double area() {
            return normal().length() / 2.0;
        }
        
        /** Returns the center of the UV coordinates */
        Vec2 textureCenter() {
            return new Vec2(
                (t[0].u + t[1].u + t[2].u) / 3.0,
                (t[0].v + t[1].v + t[2].v) / 3.0
            );
        }
        
        /** Returns the interpolated color at the triangle center (average of vertex colors) */
        int colorCenter() {
            int r0 = (c[0] >> 16) & 0xFF, g0 = (c[0] >> 8) & 0xFF, b0 = c[0] & 0xFF;
            int r1 = (c[1] >> 16) & 0xFF, g1 = (c[1] >> 8) & 0xFF, b1 = c[1] & 0xFF;
            int r2 = (c[2] >> 16) & 0xFF, g2 = (c[2] >> 8) & 0xFF, b2 = c[2] & 0xFF;
            int r = (r0 + r1 + r2) / 3;
            int g = (g0 + g1 + g2) / 3;
            int b = (b0 + b1 + b2) / 3;
            return (r << 16) | (g << 8) | b;
        }
        
        /** Interpolates between two colors by factor t (0.0 = c1, 1.0 = c2) */
        static int mixColor(int c1, int c2, double t) {
            int r1 = (c1 >> 16) & 0xFF, g1 = (c1 >> 8) & 0xFF, b1 = c1 & 0xFF;
            int r2 = (c2 >> 16) & 0xFF, g2 = (c2 >> 8) & 0xFF, b2 = c2 & 0xFF;
            int r = (int) (r1 + (r2 - r1) * t);
            int g = (int) (g1 + (g2 - g1) * t);
            int b = (int) (b1 + (b2 - b1) * t);
            return (r << 16) | (g << 8) | b;
        }
        
        /** Returns minimum on an axis */
        double min(int axis) {
            return Math.min(v[0].get(axis), Math.min(v[1].get(axis), v[2].get(axis)));
        }
        
        /** Returns maximum on an axis */
        double max(int axis) {
            return Math.max(v[0].get(axis), Math.max(v[1].get(axis), v[2].get(axis)));
        }
        
        /** Returns inclusive minimum voxel boundary */
        int[] voxelMin() {
            return new int[] {
                (int) Math.floor(min(0)),
                (int) Math.floor(min(1)),
                (int) Math.floor(min(2))
            };
        }
        
        /** Returns exclusive maximum voxel boundary */
        int[] voxelMax() {
            return new int[] {
                (int) Math.floor(max(0)) + 1,
                (int) Math.floor(max(1)) + 1,
                (int) Math.floor(max(2)) + 1
            };
        }
    }
    
    /**
     * Weighted UV for blending
     */
    private static class WeightedUv {
        double weight;
        Vec2 uv;
        
        WeightedUv(double weight, Vec2 uv) {
            this.weight = weight;
            this.uv = uv;
        }
        
        static WeightedUv mix(WeightedUv a, WeightedUv b) {
            double weightSum = a.weight + b.weight;
            if (weightSum == 0) return new WeightedUv(0, new Vec2(0, 0));
            return new WeightedUv(
                weightSum,
                new Vec2(
                    (a.weight * a.uv.u + b.weight * b.uv.u) / weightSum,
                    (a.weight * a.uv.v + b.weight * b.uv.v) / weightSum
                )
            );
        }
    }
    
    /**
     * Weighted color for blending
     */
    private static class WeightedColor {
        double weight;
        int color;
        
        WeightedColor(double weight, int color) {
            this.weight = weight;
            this.color = color;
        }
        
        /** MAX strategy: choose color with greater weight */
        static WeightedColor max(WeightedColor a, WeightedColor b) {
            return a.weight > b.weight ? a : b;
        }
    }
    
    // ==================== TRIANGLE SPLITTING ====================
    
    private static boolean isZero(double x) {
        return Math.abs(x) < EPSILON;
    }
    
    private static boolean eq(double x, int plane) {
        return isZero(x - plane);
    }
    
    /**
     * Computes the intersection parameter t for a ray with an axis-aligned plane.
     * Ray: org + t * dir
     * Plane: x[axis] = plane
     */
    private static double intersectRayAxisPlane(Vec3 org, Vec3 dir, int axis, int plane) {
        double d = -dir.get(axis);
        return isZero(d) ? 0 : (org.get(axis) - plane) / d;
    }
    
    /**
     * Signed distance from point to plane
     */
    private static double distancePointPlane(Vec3 p, Vec3 planeOrg, Vec3 planeNormal) {
        return Vec3.dot(planeNormal, Vec3.sub(p, planeOrg));
    }
    
    /**
     * Discard mode for triangle splitting
     */
    private enum DiscardMode {
        NONE,       // Keep both sides
        DISCARD_LO, // Discard triangles on the low side
        DISCARD_HI  // Discard triangles on the high side
    }
    
    /**
     * Splits a triangle against an axis-aligned plane.
     * 
     * @param axis The axis (0=X, 1=Y, 2=Z)
     * @param plane The plane position
     * @param tri The triangle to split
     * @param outLo Output for triangles on the low side (< plane)
     * @param outHi Output for triangles on the high side (>= plane)
     * @param mode Discard mode
     */
    private static void splitTriangle(int axis, int plane, TexturedTriangle tri,
                                      List<TexturedTriangle> outLo, List<TexturedTriangle> outHi,
                                      DiscardMode mode) {
        // Classify vertices
        boolean[] loVertices = new boolean[3];
        boolean[] planarVertices = new boolean[3];
        int loSum = 0, planarSum = 0;
        
        for (int i = 0; i < 3; i++) {
            double val = tri.vertex(i).get(axis);
            planarVertices[i] = eq(val, plane);
            loVertices[i] = val < plane;
            if (loVertices[i]) loSum++;
            if (planarVertices[i]) planarSum++;
        }
        
        // Helper to push to correct output based on mode
        java.util.function.BiConsumer<TexturedTriangle, Boolean> push = (t, isLo) -> {
            if (mode == DiscardMode.NONE) {
                (isLo ? outLo : outHi).add(t);
            } else if (mode == DiscardMode.DISCARD_LO) {
                if (!isLo) outHi.add(t);
            } else { // DISCARD_HI
                if (isLo) outLo.add(t);
            }
        };
        
        // Case: All vertices on hi side
        if (loSum == 0) {
            push.accept(tri, false);
            return;
        }
        
        // Case: All vertices on lo side
        if (loSum == 3) {
            push.accept(tri, true);
            return;
        }
        
        // Case: All vertices are planar (on the plane)
        if (planarSum == 3) {
            push.accept(tri, false); // Bias to hi
            return;
        }
        
        // Case: Two vertices are planar
        if (planarSum == 2) {
            // Find the non-planar vertex
            int nonPlanar = !planarVertices[0] ? 0 : !planarVertices[1] ? 1 : 2;
            push.accept(tri, loVertices[nonPlanar]);
            return;
        }
        
        // Case: One vertex is planar
        if (planarSum == 1) {
            int planarIdx = planarVertices[0] ? 0 : planarVertices[1] ? 1 : 2;
            int[] nonPlanarIdx = {(planarIdx + 1) % 3, (planarIdx + 2) % 3};
            
            // Check if both non-planar vertices are on the same side
            if (loVertices[nonPlanarIdx[0]] == loVertices[nonPlanarIdx[1]]) {
                push.accept(tri, loVertices[nonPlanarIdx[0]]);
                return;
            }
            
            // Split: one vertex on plane, other two on opposite sides
            Vec3 planarVert = tri.vertex(planarIdx);
            Vec2 planarTex = tri.texture(planarIdx);
            int planarCol = tri.color(planarIdx);
            Vec3[] nonPlanarVerts = {tri.vertex(nonPlanarIdx[0]), tri.vertex(nonPlanarIdx[1])};
            Vec2[] nonPlanarTexs = {tri.texture(nonPlanarIdx[0]), tri.texture(nonPlanarIdx[1])};
            int[] nonPlanarCols = {tri.color(nonPlanarIdx[0]), tri.color(nonPlanarIdx[1])};
            Vec3 edge = Vec3.sub(nonPlanarVerts[1], nonPlanarVerts[0]);
            
            double t = intersectRayAxisPlane(nonPlanarVerts[0], edge, axis, plane);
            Vec3 geoIsect = Vec3.mix(nonPlanarVerts[0], nonPlanarVerts[1], t);
            Vec2 texIsect = Vec2.mix(nonPlanarTexs[0], nonPlanarTexs[1], t);
            int colIsect = TexturedTriangle.mixColor(nonPlanarCols[0], nonPlanarCols[1], t);
            
            TexturedTriangle tri1 = new TexturedTriangle(planarVert, nonPlanarVerts[0], geoIsect,
                                                          planarTex, nonPlanarTexs[0], texIsect,
                                                          planarCol, nonPlanarCols[0], colIsect);
            TexturedTriangle tri2 = new TexturedTriangle(planarVert, geoIsect, nonPlanarVerts[1],
                                                          planarTex, texIsect, nonPlanarTexs[1],
                                                          planarCol, colIsect, nonPlanarCols[1]);
            
            push.accept(tri1, loVertices[nonPlanarIdx[0]]);
            push.accept(tri2, !loVertices[nonPlanarIdx[0]]);
            return;
        }
        
        // Regular case: Triangle crosses the plane, no vertices on it
        // One vertex is isolated on one side, two on the other
        boolean isolatedIsLo = loSum == 1;
        int isolatedIdx = isolatedIsLo ? 
            (loVertices[0] ? 0 : loVertices[1] ? 1 : 2) :
            (!loVertices[0] ? 0 : !loVertices[1] ? 1 : 2);
        int[] otherIdx = {(isolatedIdx + 1) % 3, (isolatedIdx + 2) % 3};
        
        Vec3 isolatedVert = tri.vertex(isolatedIdx);
        Vec2 isolatedTex = tri.texture(isolatedIdx);
        int isolatedCol = tri.color(isolatedIdx);
        Vec3[] otherVerts = {tri.vertex(otherIdx[0]), tri.vertex(otherIdx[1])};
        Vec2[] otherTexs = {tri.texture(otherIdx[0]), tri.texture(otherIdx[1])};
        int[] otherCols = {tri.color(otherIdx[0]), tri.color(otherIdx[1])};
        
        Vec3[] edges = {Vec3.sub(otherVerts[0], isolatedVert), Vec3.sub(otherVerts[1], isolatedVert)};
        double[] ts = {
            intersectRayAxisPlane(isolatedVert, edges[0], axis, plane),
            intersectRayAxisPlane(isolatedVert, edges[1], axis, plane)
        };
        Vec3[] geoIsects = {
            Vec3.mix(isolatedVert, otherVerts[0], ts[0]),
            Vec3.mix(isolatedVert, otherVerts[1], ts[1])
        };
        Vec2[] texIsects = {
            Vec2.mix(isolatedTex, otherTexs[0], ts[0]),
            Vec2.mix(isolatedTex, otherTexs[1], ts[1])
        };
        int[] colIsects = {
            TexturedTriangle.mixColor(isolatedCol, otherCols[0], ts[0]),
            TexturedTriangle.mixColor(isolatedCol, otherCols[1], ts[1])
        };
        
        // Isolated triangle
        TexturedTriangle isolatedTri = new TexturedTriangle(
            isolatedVert, geoIsects[0], geoIsects[1],
            isolatedTex, texIsects[0], texIsects[1],
            isolatedCol, colIsects[0], colIsects[1]
        );
        
        // Quad on the other side -> split into two triangles
        TexturedTriangle otherTri1 = new TexturedTriangle(
            geoIsects[0], otherVerts[0], otherVerts[1],
            texIsects[0], otherTexs[0], otherTexs[1],
            colIsects[0], otherCols[0], otherCols[1]
        );
        TexturedTriangle otherTri2 = new TexturedTriangle(
            geoIsects[0], geoIsects[1], otherVerts[1],
            texIsects[0], texIsects[1], otherTexs[1],
            colIsects[0], colIsects[1], otherCols[1]
        );
        
        push.accept(isolatedTri, isolatedIsLo);
        push.accept(otherTri1, !isolatedIsLo);
        push.accept(otherTri2, !isolatedIsLo);
    }
    
    /**
     * Computes the weighted UV for triangles inside a voxel.
     * Splits the triangle against all 6 voxel faces and returns the blended UV.
     */
    private static WeightedUv computeTriangleUvInVoxel(TexturedTriangle inputTriangle, int vx, int vy, int vz) {
        List<TexturedTriangle> preSplit = new ArrayList<>();
        List<TexturedTriangle> postSplit = new ArrayList<>();
        
        preSplit.add(inputTriangle.copy());
        
        // Split against all 6 faces of the voxel
        // hi=0: low planes (keep >= plane, discard < plane) -> DISCARD_LO
        // hi=1: high planes (keep < plane, discard >= plane) -> DISCARD_HI
        int[] pos = {vx, vy, vz};
        
        for (int hi = 0; hi < 2; hi++) {
            DiscardMode mode = hi == 0 ? DiscardMode.DISCARD_LO : DiscardMode.DISCARD_HI;
            
            for (int axis = 0; axis < 3; axis++) {
                int plane = pos[axis] + hi;
                
                for (TexturedTriangle t : preSplit) {
                    splitTriangle(axis, plane, t, postSplit, postSplit, mode);
                }
                
                preSplit.clear();
                if (postSplit.isEmpty()) {
                    return new WeightedUv(0, new Vec2(0, 0));
                }
                
                // Swap buffers
                List<TexturedTriangle> tmp = preSplit;
                preSplit = postSplit;
                postSplit = tmp;
            }
        }
        
        // preSplit now contains triangles inside the voxel
        // Blend their texture centers
        WeightedUv result = new WeightedUv(0, new Vec2(0, 0));
        for (TexturedTriangle t : preSplit) {
            double weight = inputTriangle.area(); // Use original triangle's area as weight
            Vec2 uv = t.textureCenter();
            result = WeightedUv.mix(result, new WeightedUv(weight, uv));
        }
        
        return result;
    }
    
    // ==================== MAIN VOXELIZATION ====================
    
    public static VoxelGrid voxelize(GLBParser.MeshData mesh, int resolution) {
        return voxelize(mesh, resolution, null);
    }
    
    public static VoxelGrid voxelize(GLBParser.MeshData mesh, int resolution, TextureSampler textureSampler) {
        return voxelize(mesh, resolution, textureSampler, null);
    }

    public static VoxelGrid voxelize(
            GLBParser.MeshData mesh,
            int resolution,
            TextureSampler textureSampler,
            ProgressListener progressListener
    ) {
        float[] vertices = mesh.vertices();
        int[] indices = mesh.indices();
        float[] uvs = mesh.uvs();
        
        boolean hasUVs = uvs != null && uvs.length > 0;
        boolean hasTexture = textureSampler != null && hasUVs;
        int[] colors = mesh.colors();
        boolean hasColors = colors != null && colors.length > 0;
        
        if (vertices.length == 0) {
            return new VoxelGrid(new HashMap<>(), resolution);
        }
        
        // Calculate bounding box
        double minX = Double.MAX_VALUE, minY = Double.MAX_VALUE, minZ = Double.MAX_VALUE;
        double maxX = -Double.MAX_VALUE, maxY = -Double.MAX_VALUE, maxZ = -Double.MAX_VALUE;
        
        for (int i = 0; i < vertices.length; i += 3) {
            minX = Math.min(minX, vertices[i]);
            minY = Math.min(minY, vertices[i + 1]);
            minZ = Math.min(minZ, vertices[i + 2]);
            maxX = Math.max(maxX, vertices[i]);
            maxY = Math.max(maxY, vertices[i + 1]);
            maxZ = Math.max(maxZ, vertices[i + 2]);
        }
        
        double sizeX = maxX - minX;
        double sizeY = maxY - minY;
        double sizeZ = maxZ - minZ;
        double maxSize = Math.max(Math.max(sizeX, sizeY), sizeZ);
        
        if (maxSize == 0) {
            return new VoxelGrid(new HashMap<>(), resolution);
        }
        
        // Anti-bleed: slight padding (from obj2voxel)
        double antiBleed = 0.5;
        double sampleScale = resolution - antiBleed;
        
        // Scale and offset to fit in resolution grid
        double scale = sampleScale / maxSize;
        double offsetX = -minX * scale + antiBleed / 2;
        double offsetY = -minY * scale + antiBleed / 2;
        double offsetZ = -minZ * scale + antiBleed / 2;
        
        // Build triangles
        List<TexturedTriangle> triangles = new ArrayList<>();
        for (int i = 0; i < indices.length; i += 3) {
            int idx0 = indices[i];
            int idx1 = indices[i + 1];
            int idx2 = indices[i + 2];
            
            // Transform vertices to voxel grid space
            Vec3 v0 = new Vec3(
                vertices[idx0 * 3] * scale + offsetX,
                vertices[idx0 * 3 + 1] * scale + offsetY,
                vertices[idx0 * 3 + 2] * scale + offsetZ
            );
            Vec3 v1 = new Vec3(
                vertices[idx1 * 3] * scale + offsetX,
                vertices[idx1 * 3 + 1] * scale + offsetY,
                vertices[idx1 * 3 + 2] * scale + offsetZ
            );
            Vec3 v2 = new Vec3(
                vertices[idx2 * 3] * scale + offsetX,
                vertices[idx2 * 3 + 1] * scale + offsetY,
                vertices[idx2 * 3 + 2] * scale + offsetZ
            );
            
            // Get UVs
            Vec2 t0 = hasUVs ? new Vec2(uvs[idx0 * 2], uvs[idx0 * 2 + 1]) : new Vec2(0, 0);
            Vec2 t1 = hasUVs ? new Vec2(uvs[idx1 * 2], uvs[idx1 * 2 + 1]) : new Vec2(0, 0);
            Vec2 t2 = hasUVs ? new Vec2(uvs[idx2 * 2], uvs[idx2 * 2 + 1]) : new Vec2(0, 0);
            
            // Get vertex colors
            int c0 = hasColors ? colors[idx0] : 0x808080;
            int c1 = hasColors ? colors[idx1] : 0x808080;
            int c2 = hasColors ? colors[idx2] : 0x808080;
            
            triangles.add(new TexturedTriangle(v0, v1, v2, t0, t1, t2, c0, c1, c2));
        }
        
        // Voxelize each triangle
        Map<BlockPos, WeightedColor> candidates = new HashMap<>();
        int trianglesProcessed = 0;
        int voxelsUpdated = 0;
        int totalTriangles = triangles.size();
        int progressInterval = Math.max(1, totalTriangles / 200); // ~0.5% increments
        
        for (TexturedTriangle tri : triangles) {
            trianglesProcessed++;
            
            // Get voxel bounds
            int[] vMin = tri.voxelMin();
            int[] vMax = tri.voxelMax();
            
            // Clamp to grid
            vMin[0] = Math.max(0, vMin[0]);
            vMin[1] = Math.max(0, vMin[1]);
            vMin[2] = Math.max(0, vMin[2]);
            vMax[0] = Math.min(resolution, vMax[0]);
            vMax[1] = Math.min(resolution, vMax[1]);
            vMax[2] = Math.min(resolution, vMax[2]);
            
            // Precompute plane info for distance test
            Vec3 planeOrg = tri.vertex(0);
            Vec3 planeNormal = tri.normal().normalize();
            boolean validNormal = !Double.isNaN(planeNormal.x);
            
            // Process each voxel in the bounding box
            for (int vz = vMin[2]; vz < vMax[2]; vz++) {
                for (int vy = vMin[1]; vy < vMax[1]; vy++) {
                    for (int vx = vMin[0]; vx < vMax[0]; vx++) {
                        // Distance test optimization
                        if (validNormal) {
                            Vec3 center = new Vec3(vx + 0.5, vy + 0.5, vz + 0.5);
                            double dist = Math.abs(distancePointPlane(center, planeOrg, planeNormal));
                            if (dist > DISTANCE_LIMIT) {
                                continue;
                            }
                        }
                        
                        // Split triangle against voxel and get weighted UV
                        WeightedUv weightedUv = computeTriangleUvInVoxel(tri, vx, vy, vz);
                        
                        if (weightedUv.weight <= 0) {
                            continue;
                        }
                        
                        // Sample color at the UV or use vertex colors
                        int color;
                        if (hasTexture) {
                            // Meshy-6 path: sample from texture using UV
                            color = textureSampler.sample((float) weightedUv.uv.u, (float) weightedUv.uv.v);
                        } else {
                            // SAM-3D path: use interpolated vertex colors with saturation boost
                            // SAM-3D tends to produce desaturated/pastel colors
                            color = boostSaturation(tri.colorCenter(), SATURATION_BOOST);
                        }
                        
                        // MAX strategy: triangle with larger weight wins
                        BlockPos pos = new BlockPos(vx, vy, vz);
                        WeightedColor newColor = new WeightedColor(weightedUv.weight, color);
                        WeightedColor existing = candidates.get(pos);
                        
                        if (existing == null || newColor.weight > existing.weight) {
                            candidates.put(pos, newColor);
                            voxelsUpdated++;
                        }
                    }
                }
            }
            
            if (progressListener != null &&
                    (trianglesProcessed == 1 || trianglesProcessed == totalTriangles || trianglesProcessed % progressInterval == 0)) {
                try {
                    progressListener.onProgress(trianglesProcessed, totalTriangles, candidates.size());
                } catch (Exception ignored) {
                    // Progress callbacks are non-critical.
                }
            }
        }
        
        // Extract final colors
        Map<BlockPos, Integer> voxels = new HashMap<>();
        for (Map.Entry<BlockPos, WeightedColor> entry : candidates.entrySet()) {
            voxels.put(entry.getKey(), entry.getValue().color);
        }
        
        return new VoxelGrid(voxels, resolution);
    }
    
    /**
     * Boosts the saturation of an RGB color.
     * SAM-3D tends to produce desaturated/pastel colors, so we boost them
     * to get more vibrant Minecraft blocks.
     * 
     * @param rgb The input color (packed RGB)
     * @param factor Saturation multiplier (1.0 = no change, 1.8 = 80% more saturated)
     * @return The saturated color (packed RGB)
     */
    private static int boostSaturation(int rgb, float factor) {
        // Extract RGB components
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        
        // RGB to HSL
        float max = Math.max(r, Math.max(g, b));
        float min = Math.min(r, Math.min(g, b));
        float l = (max + min) / 2f;
        
        if (max == min) {
            // Achromatic (gray) - no saturation to boost
            return rgb;
        }
        
        float d = max - min;
        float s = l > 0.5f ? d / (2f - max - min) : d / (max + min);
        
        // Calculate hue
        float h;
        if (max == r) {
            h = ((g - b) / d + (g < b ? 6f : 0f)) / 6f;
        } else if (max == g) {
            h = ((b - r) / d + 2f) / 6f;
        } else {
            h = ((r - g) / d + 4f) / 6f;
        }
        
        // Boost saturation (clamp to 1.0)
        s = Math.min(1.0f, s * factor);
        
        // HSL back to RGB
        float r2, g2, b2;
        if (s == 0) {
            r2 = g2 = b2 = l;
        } else {
            float q = l < 0.5f ? l * (1f + s) : l + s - l * s;
            float p = 2f * l - q;
            r2 = hueToRgb(p, q, h + 1f/3f);
            g2 = hueToRgb(p, q, h);
            b2 = hueToRgb(p, q, h - 1f/3f);
        }
        
        // Convert back to packed RGB
        int ri = Math.min(255, Math.max(0, Math.round(r2 * 255)));
        int gi = Math.min(255, Math.max(0, Math.round(g2 * 255)));
        int bi = Math.min(255, Math.max(0, Math.round(b2 * 255)));
        
        return (ri << 16) | (gi << 8) | bi;
    }
    
    /**
     * Helper for HSL to RGB conversion
     */
    private static float hueToRgb(float p, float q, float t) {
        if (t < 0f) t += 1f;
        if (t > 1f) t -= 1f;
        if (t < 1f/6f) return p + (q - p) * 6f * t;
        if (t < 1f/2f) return q;
        if (t < 2f/3f) return p + (q - p) * (2f/3f - t) * 6f;
        return p;
    }
}
