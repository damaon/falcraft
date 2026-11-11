<script lang="ts">
	import icon from '#lib/assets/roblox/let-it-plow-icon.jpg';
	import ShortEmbed from './ShortEmbed.svelte';

	import { ROBLOX_GAME_URL as GAME_URL } from './links';
	const SHORT_ID = 'vKJ21e6pyTA';
</script>

<section class="wrap roblox" id="roblox" aria-labelledby="roblox-title">
	<h2 class="section-title">🎮 Our Roblox game</h2>
	<div class="card">
		<div class="video">
			<ShortEmbed id={SHORT_ID} title="Let it Plow: Snow Simulator on Roblox" poster={icon} />
		</div>
		<div class="info">
			<div class="head">
				<img class="icon" src={icon} alt="" width="96" height="96" />
				<div>
					<span class="badge">New on Roblox ❄️</span>
					<h3 id="roblox-title">Let it Plow: Snow Simulator</h3>
				</div>
			</div>
			<p class="pitch">
				The whole town is buried in snow right before Christmas! Grab a shovel, clear the snow and become the
				best snowplow driver in the village.
			</p>
			<ul>
				<li><span aria-hidden="true">⛄</span> Scoop up snow and sell it for gingerbread cookies</li>
				<li><span aria-hidden="true">🚜</span> Upgrade from a tiny shovel to a giant snowplow truck</li>
				<li><span aria-hidden="true">✨</span> Clear whole streets and watch the Christmas lights turn on</li>
				<li><span aria-hidden="true">👫</span> Play with friends on phone, tablet or computer</li>
			</ul>
			<a class="btn play" href={GAME_URL} target="_blank" rel="noopener noreferrer">▶ Play on Roblox</a>
		</div>
	</div>
</section>

<style>
	.roblox {
		margin-top: 56px;
	}
	.section-title {
		margin-bottom: 18px;
		font-size: clamp(34px, 6vw, 52px);
		text-align: center;
		color: #fff;
		text-shadow: 0 4px 0 #22184f;
		-webkit-text-stroke: 1.5px #22184f;
		paint-order: stroke fill;
	}
	.card {
		display: grid;
		grid-template-columns: minmax(220px, 300px) 1fr;
		gap: 32px;
		align-items: center;
		padding: 28px;
		border-radius: var(--radius);
		background:
			radial-gradient(circle at 85% 10%, rgba(255, 255, 255, 0.9) 0 2px, transparent 3px) 0 0 / 46px 46px,
			linear-gradient(160deg, #e9f6ff, #ffffff 60%);
		box-shadow: var(--shadow);
		color: #22184f;
	}
	.video {
		width: 100%;
		max-width: 300px;
		justify-self: center;
	}
	.head {
		display: flex;
		align-items: center;
		gap: 16px;
	}
	.icon {
		width: 96px;
		height: 96px;
		border-radius: 22px;
		box-shadow: 0 5px 0 rgba(34, 24, 79, 0.2);
	}
	.badge {
		display: inline-block;
		margin-bottom: 6px;
		padding: 4px 12px;
		border-radius: 999px;
		font: 16px/1.2 var(--font-display);
		background: #4cc9f0;
		color: #fff;
	}
	h3 {
		font-size: clamp(28px, 4vw, 40px);
	}
	.pitch {
		margin: 18px 0 12px;
		font-size: 19px;
	}
	ul {
		display: grid;
		gap: 8px;
		margin: 0 0 22px;
		padding: 0;
		list-style: none;
	}
	li {
		display: flex;
		gap: 10px;
		font-weight: 700;
	}
	.play {
		background: #00b06f;
		color: #fff;
		box-shadow: 0 5px 0 #007a4c;
		font-size: 22px;
		min-height: 56px;
		padding-inline: 28px;
	}
	@media (max-width: 720px) {
		.card {
			grid-template-columns: 1fr;
			padding: 20px;
			gap: 24px;
		}
		.play {
			width: 100%;
		}
	}
</style>
