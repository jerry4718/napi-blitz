import { bootstrap } from './src/main.ts'

process.on("uncaughtException", (err, origin) => {
    console.error(`Uncaught#${ origin }:`, err);
});

bootstrap().catch((err) => {
    console.error('[multi-window] bootstrap failed:', err)
    process.exit(1)
})
