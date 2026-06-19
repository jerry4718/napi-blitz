const fs = require('node:fs');

const declarationPath = process.argv[2] ?? 'dist/native.d.ts';
const declaration = fs.readFileSync(declarationPath, 'utf8');

fs.writeFileSync(
  declarationPath,
  declaration.replace(/from "\.\.\/\.\.\/native"/g, 'from "../native"'),
);
