export interface Vec3 {
  x: number
  y: number
  z: number
}

export type TriangleFace = [Vec3, Vec3, Vec3]

export function generateIcosaSphere(level: number): TriangleFace[] {
  const t = (1 + Math.sqrt(5)) / 2

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

function midpoint(a: Vec3, b: Vec3): Vec3 {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2, z: (a.z + b.z) / 2 }
}

function normalize(v: Vec3): Vec3 {
  const len = Math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z) || 1
  return { x: v.x / len, y: v.y / len, z: v.z / len }
}
