<script lang="ts">
	import '../app.css';
	// Shared with the video renderer (voxreel/assets).
	import logo from '../../../assets/logo.svg';
	import Socials from '#lib/Socials.svelte';
	import { ROBLOX_GAME_URL } from '#lib/links.ts';
	import { resolve } from '$app/paths';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" href={logo} type="image/svg+xml" />
	<meta name="description" content="Three builders, one idea. Watch the builds come together block by block and pick your favorite!" />
</svelte:head>

<header class="wrap top">
	<a class="logo" href={resolve('/')}>
		<img src={logo} alt="" width="56" height="56" />
		<span>Bananini Games</span>
	</a>
	<div class="top-links">
		<a class="roblox" href={ROBLOX_GAME_URL} target="_blank" rel="noopener noreferrer">
			<span aria-hidden="true">🎮</span> Play on Roblox
		</a>
		<Socials compact />
	</div>
</header>

<main>
	{@render children()}
</main>

<footer class="wrap foot">
	<Socials />
	<p class="made">Bananini Games · made block by block 🍌</p>
	<p class="legal">Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.</p>
	<p class="legal">Roblox is a trademark of Roblox Corporation; this site is not affiliated with Roblox.</p>
</footer>

<style>
	.top {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding-block: 18px 6px;
	}
	.top-links {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.roblox {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		min-height: 48px;
		padding: 10px 20px;
		border-radius: 999px;
		font: 19px/1 var(--font-display);
		color: #fff;
		background: #00b06f;
		box-shadow: 0 5px 0 #007a4c;
		text-decoration: none;
		transition: transform 0.08s ease;
	}
	.roblox:hover {
		transform: translateY(-2px);
	}
	.roblox:focus-visible {
		outline: 4px solid var(--accent);
		outline-offset: 3px;
	}
	@media (prefers-reduced-motion: reduce) {
		.roblox {
			transition: none;
		}
		.roblox:hover {
			transform: none;
		}
	}
	.made {
		margin-top: 24px !important;
	}
	.logo {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		font: 28px/1 var(--font-display);
		text-decoration: none;
		color: #fff;
		text-shadow: 0 3px 0 rgba(34, 24, 79, 0.35);
	}
	.foot {
		padding-block: 40px 32px;
		text-align: center;
		color: var(--ink-soft);
	}
	.foot p {
		margin: 0 0 6px;
	}
	.legal {
		font-size: 14px;
	}
</style>
