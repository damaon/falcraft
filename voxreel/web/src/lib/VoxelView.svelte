<script lang="ts">
	import { onMount } from 'svelte';
	import { VoxelStage } from './stage';
	import { THEMES, type ThemeName } from './themes';
	import type { Build } from './voxels';

	interface Props {
		builds: Build[];
		theme: ThemeName;
		interactive?: boolean;
		/** Increment to replay the build animation. */
		playKey?: number;
		label?: string;
		ondone?: () => void;
	}

	let { builds, theme, interactive = false, playKey = 0, label = 'Voxel build', ondone }: Props = $props();

	let canvas: HTMLCanvasElement;
	let stage: VoxelStage | null = null;
	let failed = $state(false);

	onMount(() => {
		const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
		try {
			stage = new VoxelStage(canvas, THEMES[theme], { interactive, autoRotate: !reduced });
			stage.setBuilds(builds);
		} catch (e) {
			console.error('WebGL unavailable', e);
			failed = true;
		}
		return () => stage?.dispose();
	});

	$effect(() => {
		if (playKey > 0) stage?.play(undefined, ondone);
	});
</script>

<div class="view">
	<canvas bind:this={canvas} aria-label={label}></canvas>
	{#if failed}
		<p class="fallback">Your browser can't show 3D here 😢</p>
	{/if}
</div>

<style>
	.view {
		position: relative;
		width: 100%;
		height: 100%;
	}
	canvas {
		display: block;
		width: 100%;
		height: 100%;
		border-radius: inherit;
	}
	.fallback {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		margin: 0;
		font-family: var(--font-display);
		color: var(--ink);
	}
</style>
