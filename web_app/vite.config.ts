import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import wasm from "vite-plugin-wasm";
import { VitePWA } from 'vite-plugin-pwa';

export default defineConfig({
    plugins: [
        react(),
        wasm(),
        VitePWA({
            registerType: 'autoUpdate',
            includeAssets: ['favicon.ico'],
            workbox: {
                maximumFileSizeToCacheInBytes: 3 * 1024 ** 2,
            },
            manifest: {
                name: 'SantoriniAI',
                short_name: 'SantoriniAI',
                theme_color: '#cfecf7',
                icons: [
                    {
                        src: 'img-512x512.png',
                        sizes: '512x512',
                        type: 'image/png',
                        purpose: 'any'
                    },
                ],
            },
        })
    ],
    worker: {
        format: "es",
    },
})
