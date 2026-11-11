<script lang="ts">
	import { resolve } from '$app/paths';
	import { battles } from '#lib/builds.ts';
	import { buildFromSpec } from '#lib/voxels.ts';
	import VoxelView from '#lib/VoxelView.svelte';
	import { DROP_SECS } from '#lib/stage.ts';

	const cards = battles.map((b) => ({ ...b, voxelBuilds: b.builds.map((s) => buildFromSpec(s, 6, DROP_SECS)) }));
</script>

<svelte:head>
	<title>Build Battle</title>
</svelte:head>

<section class="wrap hero">
	<h1>Build Battle!</h1>
	<p>3 builders. 1 idea. Watch them build it block by block, then pick your favorite!</p>
</section>

<section class="wrap grid" aria-label="Battles">
	{#each cards as b (b.slug)}
		<a class="card" href={resolve('/b/[slug]', { slug: b.slug })}>
			<div class="scene">
				<VoxelView builds={b.voxelBuilds} theme={b.theme} label="{b.title}: all {b.builds.length} builds" />
			</div>
			<div class="body">
				<h2><span aria-hidden="true">{b.emoji}</span> {b.title}</h2>
				<p>{b.tagline}</p>
				<span class="btn">Watch the battle ▶</span>
			</div>
		</a>
	{/each}
</section>

<style>
	.hero {
		padding-block: 28px 32px;
		text-align: center;
		color: #fff;
	}
	.hero h1 {
		font-size: clamp(48px, 11vw, 96px);
		text-shadow:
			0 5px 0 #22184f,
			0 10px 24px rgba(34, 24, 79, 0.35);
		-webkit-text-stroke: 2px #22184f;
		paint-order: stroke fill;
	}
	.hero p {
		max-width: 34ch;
		margin: 14px auto 0;
		font-size: 21px;
		font-weight: 700;
		text-shadow: 0 2px 0 rgba(34, 24, 79, 0.35);
	}
	.grid {
		display: grid;
		gap: 28px;
		grid-template-columns: repeat(auto-fill, minmax(min(100%, 340px), 1fr));
	}
	.card {
		display: flex;
		flex-direction: column;
		background: var(--surface);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		text-decoration: none;
		overflow: hidden;
		transition: transform 0.15s ease;
	}
	.card:hover {
		transform: translateY(-4px) rotate(-0.4deg);
	}
	.card:focus-visible {
		outline: 4px solid var(--accent);
		outline-offset: 4px;
	}
	.scene {
		aspect-ratio: 16 / 10;
		pointer-events: none;
	}
	.body {
		display: grid;
		gap: 10px;
		justify-items: start;
		padding: 18px 20px 22px;
	}
	.body h2 {
		font-size: 32px;
	}
	.body p {
		margin: 0;
		color: var(--ink-soft);
	}
	@media (prefers-reduced-motion: reduce) {
		.card {
			transition: none;
		}
		.card:hover {
			transform: none;
		}
	}
</style>
