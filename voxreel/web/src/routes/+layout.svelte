<script lang="ts">
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import { resolve } from '$app/paths';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<meta name="description" content="Three builders, one idea. Watch the builds come together block by block and pick your favorite!" />
</svelte:head>

<header class="wrap top">
	<a class="logo" href={resolve('/')}>🧱 Build Battle</a>
</header>

<main>
	{@render children()}
</main>

<footer class="wrap foot">Made block by block 🧱</footer>

<style>
	.top {
		padding-block: 18px 6px;
	}
	.logo {
		font: 26px/1 var(--font-display);
		text-decoration: none;
		color: #fff;
		text-shadow: 0 3px 0 rgba(34, 24, 79, 0.35);
	}
	.foot {
		padding-block: 40px 32px;
		text-align: center;
		color: var(--ink-soft);
	}
</style>
