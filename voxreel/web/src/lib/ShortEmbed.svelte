<script lang="ts">
	// Click-to-load YouTube Short: nothing is requested from YouTube until the viewer taps play
	// (kids' site, so no tracking on page load), then the privacy-enhanced player takes over.
	interface Props {
		id: string;
		title: string;
		poster: string;
	}
	let { id, title, poster }: Props = $props();
	let playing = $state(false);
</script>

<div class="short">
	{#if playing}
		<iframe
			src="https://www.youtube-nocookie.com/embed/{id}?autoplay=1&rel=0&playsinline=1"
			{title}
			allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
			allowfullscreen
		></iframe>
	{:else}
		<button class="poster" onclick={() => (playing = true)} aria-label="Play video: {title}">
			<img src={poster} alt="" loading="lazy" />
			<span class="play" aria-hidden="true">
				<svg viewBox="0 0 24 24"><path fill="currentColor" d="M8 5.5v13l11-6.5z" /></svg>
			</span>
			<span class="label">Watch the short</span>
		</button>
	{/if}
</div>

<style>
	.short {
		position: relative;
		aspect-ratio: 9 / 16;
		width: 100%;
		border-radius: 22px;
		overflow: hidden;
		background: #0b0b14;
		box-shadow: 0 6px 0 rgba(34, 24, 79, 0.25);
	}
	iframe,
	.poster {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		border: 0;
	}
	.poster {
		padding: 0;
		cursor: pointer;
		background: #0b0b14;
	}
	.poster img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		filter: saturate(1.1);
	}
	.play {
		position: absolute;
		top: 50%;
		left: 50%;
		display: grid;
		place-items: center;
		width: 84px;
		height: 84px;
		margin: -42px 0 0 -42px;
		border-radius: 50%;
		color: #fff;
		background: #ff0033;
		box-shadow: 0 6px 0 #a3001f;
		transition: transform 0.12s ease;
	}
	.play svg {
		width: 44px;
		height: 44px;
		margin-left: 4px;
	}
	.poster:hover .play {
		transform: scale(1.08);
	}
	.poster:focus-visible {
		outline: 4px solid var(--accent);
		outline-offset: -4px;
	}
	.label {
		position: absolute;
		left: 0;
		right: 0;
		bottom: 18px;
		font: 22px/1 var(--font-display);
		color: #fff;
		text-shadow: 0 3px 0 rgba(0, 0, 0, 0.5);
	}
	@media (prefers-reduced-motion: reduce) {
		.play {
			transition: none;
		}
	}
</style>
