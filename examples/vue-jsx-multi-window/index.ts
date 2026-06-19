import { bootstrap } from './src/main.ts'

bootstrap().catch((err) => {
    console.error('[multi-window] bootstrap failed:', err)
    process.exit(1)
})
