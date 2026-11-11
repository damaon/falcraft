<script lang="ts">
	interface Props {
		/** Icons only (header) vs. icon + label buttons (footer). */
		compact?: boolean;
	}
	let { compact = false }: Props = $props();

	const links = [
		{ name: 'YouTube', label: 'Watch on YouTube', href: 'https://www.youtube.com/@BananiniGames', bg: '#ff0033' },
		{ name: 'TikTok', label: 'Follow on TikTok', href: 'https://www.tiktok.com/@bananinigames', bg: '#111111' }
	];
</script>

<nav class="socials" class:compact aria-label="Bananini Games on social media">
	{#each links as l (l.name)}
		<a href={l.href} target="_blank" rel="noopener noreferrer" style:--bg={l.bg} aria-label={compact ? `Bananini Games on ${l.name}` : undefined}>
			{#if l.name === 'YouTube'}
				<svg viewBox="0 0 24 24" aria-hidden="true"
					><path
						fill="currentColor"
						d="M23.5 6.5a3 3 0 0 0-2.1-2.1C19.5 4 12 4 12 4s-7.5 0-9.4.4A3 3 0 0 0 .5 6.5 31 31 0 0 0 0 12a31 31 0 0 0 .5 5.5 3 3 0 0 0 2.1 2.1c1.9.4 9.4.4 9.4.4s7.5 0 9.4-.4a3 3 0 0 0 2.1-2.1A31 31 0 0 0 24 12a31 31 0 0 0-.5-5.5ZM9.6 15.6V8.4l6.3 3.6-6.3 3.6Z"
					/></svg
				>
			{:else}
				<svg viewBox="0 0 24 24" aria-hidden="true"
					><path
						fill="currentColor"
						d="M16.6 2h-3.4v13.4a2.9 2.9 0 1 1-2.9-2.9c.3 0 .6 0 .9.1V9.2a6.3 6.3 0 1 0 5.4 6.2V8.6a8.1 8.1 0 0 0 4.7 1.5V6.7A4.7 4.7 0 0 1 16.6 2Z"
					/></svg
				>
			{/if}
			{#if !compact}<span>{l.label}</span>{/if}
		</a>
	{/each}
</nav>

<style>
	.socials {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		justify-content: center;
	}
	a {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		min-height: 48px;
		padding: 10px 20px;
		border-radius: 999px;
		background: var(--bg);
		color: #fff;
		font: 19px/1 var(--font-display);
		text-decoration: none;
		box-shadow: 0 5px 0 rgba(34, 24, 79, 0.35);
		transition: transform 0.08s ease;
	}
	a:hover {
		transform: translateY(-2px);
	}
	a:focus-visible {
		outline: 4px solid var(--accent);
		outline-offset: 3px;
	}
	svg {
		width: 26px;
		height: 26px;
		flex: none;
	}
	.compact a {
		width: 48px;
		height: 48px;
		padding: 0;
		justify-content: center;
	}
	.compact svg {
		width: 24px;
		height: 24px;
	}
	@media (prefers-reduced-motion: reduce) {
		a {
			transition: none;
		}
		a:hover {
			transform: none;
		}
	}
</style>
