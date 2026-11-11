<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { buildFromSpec } from '#lib/voxels.ts';
	import { DROP_SECS } from '#lib/stage.ts';
	import VoxelView from '#lib/VoxelView.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const battle = $derived(data.battle);
	const builds = $derived(battle.builds.map((s) => buildFromSpec(s, 6, DROP_SECS)));

	// One replay counter per build; "Build all" bumps every one of them.
	let plays = $state<number[]>([]);
	let building = $state<boolean[]>([]);
	let vote = $state<number | null>(null);

	$effect(() => {
		plays = builds.map(() => 0);
		building = builds.map(() => false);
		vote = readVote(battle.slug);
	});

	function storageKey(slug: string) {
		return `build-battle:vote:${slug}`;
	}

	function readVote(slug: string): number | null {
		try {
			const v = localStorage.getItem(storageKey(slug));
			return v === null ? null : Number(v);
		} catch {
			return null;
		}
	}

	function pick(i: number) {
		vote = i;
		try {
			localStorage.setItem(storageKey(battle.slug), String(i));
		} catch {
			// Private mode or blocked storage: the pick still shows for this visit.
		}
	}

	function play(i: number) {
		plays[i] += 1;
		building[i] = true;
	}

	function playAll() {
		builds.forEach((_, i) => play(i));
	}

	// Shareable "watch it build" links: /b/<slug>#play starts every build on load.
	onMount(() => {
		if (location.hash === '#play') requestAnimationFrame(playAll);
	});
</script>

<svelte:head>
	<title>{battle.title}: Build Battle</title>
</svelte:head>

<section class="wrap head">
	<a class="back" href={resolve('/')}>← All battles</a>
	<h1><span aria-hidden="true">{battle.emoji}</span> {battle.title}</h1>
	<p>{battle.tagline}</p>
	<button class="btn pink" onclick={playAll}>▶ Build them all!</button>
</section>

<section class="wrap builds" aria-label="Builds">
	{#each builds as build, i (i)}
		<article class="build" class:picked={vote === i}>
			<div class="badge" aria-hidden="true">{i + 1}</div>
			<div class="scene">
				<VoxelView
					builds={[build]}
					theme={battle.theme}
					interactive
					playKey={plays[i] ?? 0}
					label="{battle.title} build number {i + 1}. Drag to spin it."
					ondone={() => (building[i] = false)}
				/>
			</div>
			<div class="meta">
				<span class="count">{build.voxels.length.toLocaleString('en')} blocks</span>
				<div class="actions">
					<button class="btn" onclick={() => play(i)} disabled={building[i]}>
						{building[i] ? 'Building…' : '▶ Build it!'}
					</button>
					<button class="btn green" onclick={() => pick(i)} aria-pressed={vote === i}>
						{vote === i ? '⭐ My pick!' : `Pick #${i + 1}`}
					</button>
				</div>
			</div>
		</article>
	{/each}
</section>

{#if vote !== null}
	<p class="wrap thanks" role="status">You picked #{vote + 1}! 🎉 Drag the builds to spin them around.</p>
{/if}

{#if data.others.length}
	<section class="wrap more">
		<h2>More battles</h2>
		<div class="links">
			{#each data.others as o (o.slug)}
				<a class="btn" href={resolve('/b/[slug]', { slug: o.slug })}>{o.emoji} {o.title}</a>
			{/each}
		</div>
	</section>
{/if}

<style>
	.head {
		display: grid;
		justify-items: center;
		gap: 12px;
		padding-block: 8px 28px;
		text-align: center;
		color: #fff;
	}
	.back {
		justify-self: start;
		font-weight: 700;
		text-decoration: none;
		text-shadow: 0 2px 0 rgba(34, 24, 79, 0.35);
	}
	.head h1 {
		font-size: clamp(44px, 10vw, 84px);
		text-shadow: 0 5px 0 #22184f;
		-webkit-text-stroke: 2px #22184f;
		paint-order: stroke fill;
	}
	.head p {
		margin: 0;
		font-size: 21px;
		font-weight: 700;
		text-shadow: 0 2px 0 rgba(34, 24, 79, 0.35);
	}
	.builds {
		display: grid;
		gap: 28px;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr));
	}
	.build {
		position: relative;
		display: flex;
		flex-direction: column;
		background: var(--surface);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		outline: 5px solid transparent;
		transition: outline-color 0.15s ease;
	}
	.build.picked {
		outline-color: var(--accent);
	}
	.badge {
		position: absolute;
		z-index: 1;
		top: -14px;
		left: -10px;
		display: grid;
		place-items: center;
		width: 64px;
		height: 64px;
		border-radius: 50%;
		font: 40px/1 var(--font-display);
		color: var(--accent-ink);
		background: var(--accent);
		box-shadow: 0 4px 0 #c9a400;
	}
	.scene {
		aspect-ratio: 4 / 5;
		border-radius: var(--radius) var(--radius) 0 0;
		overflow: hidden;
		touch-action: pan-y;
	}
	.meta {
		display: grid;
		gap: 12px;
		padding: 16px 18px 20px;
	}
	.count {
		font: 22px/1 var(--font-display);
		color: var(--ink-soft);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 10px;
	}
	.actions .btn {
		flex: 1 1 130px;
	}
	.btn:disabled {
		opacity: 0.6;
		cursor: default;
	}
	.thanks {
		margin-top: 28px;
		padding: 16px 20px;
		border-radius: var(--radius);
		background: var(--surface);
		box-shadow: var(--shadow);
		font: 24px/1.2 var(--font-display);
		text-align: center;
	}
	.more {
		margin-top: 48px;
		display: grid;
		gap: 16px;
		justify-items: center;
	}
	.more h2 {
		font-size: 36px;
		color: #fff;
		text-shadow: 0 3px 0 #22184f;
	}
	.links {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		justify-content: center;
	}
	@media (prefers-reduced-motion: reduce) {
		.build {
			transition: none;
		}
	}
</style>
