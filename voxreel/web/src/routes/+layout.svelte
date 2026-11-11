<script lang="ts">
	import '../app.css';
	import logo from '#lib/assets/logo.svg';
	import Socials from '#lib/Socials.svelte';
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
	<Socials compact />
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
