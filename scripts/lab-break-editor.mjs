import { readFileSync, writeFileSync } from "node:fs";

const file = process.argv[2];
const text = readFileSync(file, "utf8");
writeFileSync(file, text.replace(/^pick /m, "break "));
