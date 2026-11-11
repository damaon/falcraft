import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},

			// Fully static output (build/): host it anywhere, e.g. GitHub Pages or any CDN.
			adapter: adapter()
		})
	],
	server: {
		// Builds (../builds) and the shared font (../assets) live next to the web app.
		fs: { allow: ['..'] }
	}
});
