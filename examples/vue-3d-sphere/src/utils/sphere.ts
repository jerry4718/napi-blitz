/**
 * Sphere geometry generator using UV-sphere subdivision.
 *
 * Returns a flat array of faces. Each face is an array of 3 {x,y,z} vertices
 * (triangles), suitable for projecting via CSS 3D transforms onto <div> faces.
 */

export interface Vec3 {
  x: number
  y: number
  z: number
}

export type TriangleFace = [Vec3, Vec3, Vec3]

/**
 * Build a UV-sphere with the given subdivision level.
 *
 * Level 0 → octahedron (8 faces)
 * Level 1 → 32 faces
 * Level 2 → 128 faces
 * Level N → 8 × 4^N faces
 */
export function generateSphere(level: number): TriangleFace[] {
  // ---------- octahedron base (8 triangles) ----------
  const top: Vec3 = { x: 0, y: 1, z: 0 }
  const bottom: Vec3 = { x: 0, y: -1, z: 0 }
  const front: Vec3 = { x: 0, y: 0, z: 1 }
  const back: Vec3 = { x: 0, y: 0, z: -1 }
  const left: Vec3 = { x: -1, y: 0, z: 0 }
  const right: Vec3 = { x: 1, y: 0, z: 0 }

  const baseTris: TriangleFace[] = [
    [top, front, right],
    [top, right, back],
    [top, back, left],
    [top, left, front],
    [bottom, right, front],
    [bottom, back, right],
    [bottom, left, back],
    [bottom, front, left],
  ]

  let faces = baseTris

  // Subdivide each triangle into 4 smaller triangles.
  for (let i = 0; i < level; i++) {
    const subdivided: TriangleFace[] = []
    for (const [a, b, c] of faces) {
      // Midpoints on the unit sphere surface.
      const ab = normalize(midpoint(a, b))
      const bc = normalize(midpoint(b, c))
      const ca = normalize(midpoint(c, a))
      subdivided.push([a, ab, ca])
      subdivided.push([b, bc, ab])
      subdivided.push([c, ca, bc])
      subdivided.push([ab, bc, ca])
    }
    faces = subdivided
  }

  return faces
}

/** Unit-sphere icosahedron base — smoother starting shape, 20 faces. */
export function generateIcosaSphere(level: number): TriangleFace[] {
  const t = (1 + Math.sqrt(5)) / 2 // golden ratio

  // 12 vertices of a unit icosahedron.
  const verts: Vec3[] = [
    normalize({ x: -1, y: t, z: 0 }),
    normalize({ x: 1, y: t, z: 0 }),
    normalize({ x: -1, y: -t, z: 0 }),
    normalize({ x: 1, y: -t, z: 0 }),
    normalize({ x: 0, y: -1, z: t }),
    normalize({ x: 0, y: 1, z: t }),
    normalize({ x: 0, y: -1, z: -t }),
    normalize({ x: 0, y: 1, z: -t }),
    normalize({ x: t, y: 0, z: -1 }),
    normalize({ x: t, y: 0, z: 1 }),
    normalize({ x: -t, y: 0, z: -1 }),
    normalize({ x: -t, y: 0, z: 1 }),
  ]

  // 20 triangular faces (indices into verts).
  const idxTris: [number, number, number][] = [
    [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
    [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
    [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
    [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
  ]

  let faces: TriangleFace[] = idxTris.map(
    ([a, b, c]) => [verts[a], verts[b], verts[c]],
  )

  for (let i = 0; i < level; i++) {
    const subdivided: TriangleFace[] = []
    for (const [a, b, c] of faces) {
      const ab = normalize(midpoint(a, b))
      const bc = normalize(midpoint(b, c))
      const ca = normalize(midpoint(c, a))
      subdivided.push([a, ab, ca])
      subdivided.push([b, bc, ab])
      subdivided.push([c, ca, bc])
      subdivided.push([ab, bc, ca])
    }
    faces = subdivided
  }

  return faces
}

// ---------- helpers ----------

function midpoint(a: Vec3, b: Vec3): Vec3 {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2, z: (a.z + b.z) / 2 }
}

function normalize(v: Vec3): Vec3 {
  const len = Math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z) || 1
  return { x: v.x / len, y: v.y / len, z: v.z / len }
}
