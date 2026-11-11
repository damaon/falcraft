<script lang="ts">
	// Header mascot: the eyes follow the cursor, and a click sends it hopping across the cards
	// on the page, smashing each one into flying blocks (they pop back once it is home).
	import { onMount } from 'svelte';
	// Shared with the video renderer; the pupils carry class="pupil".
	import logoSvg from '../../../assets/logo.svg?raw';

	const TARGETS = '.card, .build';
	const BLOCK_COLORS = ['#ffd60a', '#f2b705', '#7a4a2a', '#6cbf4a', '#3aa6e0', '#ff7a9a', '#2f63c9'];

	let host: HTMLButtonElement;
	let busy = $state(false);

	onMount(() => {
		const pupils = [...host.querySelectorAll<SVGElement>('.pupil')];
		let frame = 0;
		const look = (e: PointerEvent) => {
			cancelAnimationFrame(frame);
			frame = requestAnimationFrame(() => {
				const r = host.getBoundingClientRect();
				// Midpoint between the eyes is at (64, 43) of the 128 viewBox.
				const dx = e.clientX - (r.left + (64 / 128) * r.width);
				const dy = e.clientY - (r.top + (43 / 128) * r.height);
				const len = Math.hypot(dx, dy) || 1;
				const k = Math.min(1, len / 120) * 2.6;
				// Pupils are drawn 1.5/1 units off the eye centre; recentre, then look.
				const t = `translate(${(dx / len) * k - 1.5}px, ${(dy / len) * k - 1}px)`;
				for (const p of pupils) p.style.transform = t;
			});
		};
		window.addEventListener('pointermove', look, { passive: true });
		return () => {
			window.removeEventListener('pointermove', look);
			cancelAnimationFrame(frame);
		};
	});

	const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

	function inView(el: Element) {
		const r = el.getBoundingClientRect();
		return r.bottom > 60 && r.top < innerHeight - 60 && r.width > 0;
	}

	/** Parabolic hop of the flying monkey from its current spot to (x, y) (viewport, centre). */
	function hop(monkey: HTMLElement, from: { x: number; y: number }, to: { x: number; y: number }, ms: number) {
		const steps = 16;
		const peak = Math.max(90, Math.abs(to.x - from.x) * 0.35);
		const frames: Keyframe[] = [];
		for (let i = 0; i <= steps; i++) {
			const t = i / steps;
			const x = from.x + (to.x - from.x) * t;
			const y = from.y + (to.y - from.y) * t - peak * 4 * t * (1 - t);
			const spin = (to.x >= from.x ? 1 : -1) * 360 * t;
			frames.push({ transform: `translate(${x}px, ${y}px) translate(-50%, -50%) rotate(${spin}deg)` });
		}
		return monkey.animate(frames, { duration: ms, easing: 'linear', fill: 'forwards' }).finished;
	}

	function burst(x: number, y: number) {
		for (let i = 0; i < 18; i++) {
			const b = document.createElement('div');
			b.className = 'egg-block';
			b.style.background = BLOCK_COLORS[i % BLOCK_COLORS.length];
			b.style.left = `${x}px`;
			b.style.top = `${y}px`;
			document.body.append(b);
			const a = Math.random() * Math.PI * 2;
			const v = 120 + Math.random() * 220;
			const dx = Math.cos(a) * v;
			const dy = Math.sin(a) * v - 160;
			b.animate(
				[
					{ transform: 'translate(-50%, -50%) rotate(0deg)', opacity: 1 },
					{ transform: `translate(calc(-50% + ${dx}px), calc(-50% + ${dy}px)) rotate(${Math.random() * 540}deg)`, opacity: 1, offset: 0.45 },
					{ transform: `translate(calc(-50% + ${dx * 1.4}px), calc(-50% + ${dy + 700}px)) rotate(${Math.random() * 900}deg)`, opacity: 0 }
				],
				{ duration: 1100 + Math.random() * 400, easing: 'cubic-bezier(.3,.6,.6,1)' }
			).finished.then(() => b.remove());
		}
		const bonk = document.createElement('div');
		bonk.className = 'egg-bonk';
		bonk.textContent = ['BONK!', 'SMASH!', 'POW!', 'CRUNCH!'][Math.floor(Math.random() * 4)];
		bonk.style.left = `${x}px`;
		bonk.style.top = `${y}px`;
		document.body.append(bonk);
		bonk
			.animate(
				[
					{ transform: 'translate(-50%, -50%) scale(0.2) rotate(-12deg)', opacity: 0 },
					{ transform: 'translate(-50%, -120%) scale(1.15) rotate(-6deg)', opacity: 1, offset: 0.25 },
					{ transform: 'translate(-50%, -190%) scale(1) rotate(-6deg)', opacity: 0 }
				],
				{ duration: 900, easing: 'ease-out' }
			)
			.finished.then(() => bonk.remove());
	}

	async function smash(card: HTMLElement) {
		await card.animate(
			[{ transform: 'translateX(0)' }, { transform: 'translateX(-8px) rotate(-1.5deg)' }, { transform: 'translateX(8px) rotate(1.5deg)' }, { transform: 'translateX(0)' }],
			{ duration: 220, iterations: 2 }
		).finished;
		const r = card.getBoundingClientRect();
		burst(r.left + r.width / 2, r.top + r.height / 3);
		const dir = Math.random() < 0.5 ? -1 : 1;
		const fall = card.animate(
			[
				{ transform: 'none', opacity: 1 },
				{ transform: `translateY(-30px) rotate(${dir * 6}deg) scale(0.95)`, opacity: 1, offset: 0.2 },
				{ transform: `translateY(${innerHeight}px) rotate(${dir * 40}deg) scale(0.6)`, opacity: 0 }
			],
			{ duration: 900, easing: 'cubic-bezier(.5,0,.9,.6)', fill: 'forwards' }
		);
		return fall;
	}

	async function rampage() {
		if (busy) return;
		if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
			host.animate([{ transform: 'rotate(0)' }, { transform: 'rotate(-12deg)' }, { transform: 'rotate(12deg)' }, { transform: 'rotate(0)' }], 400);
			return;
		}
		busy = true;
		const cards = [...document.querySelectorAll<HTMLElement>(TARGETS)].filter(inView);
		const home = host.getBoundingClientRect();
		const start = { x: home.left + home.width / 2, y: home.top + home.height / 2 };

		const monkey = document.createElement('div');
		monkey.className = 'egg-monkey';
		monkey.innerHTML = logoSvg;
		monkey.setAttribute('aria-hidden', 'true');
		monkey.style.transform = `translate(${start.x}px, ${start.y}px) translate(-50%, -50%)`;
		document.body.append(monkey);
		host.style.visibility = 'hidden';

		const falls: Animation[] = [];
		let at = start;
		try {
			for (const card of cards) {
				const r = card.getBoundingClientRect();
				const to = { x: r.left + r.width / 2, y: r.top + Math.min(70, r.height / 4) };
				await hop(monkey, at, to, 650);
				at = to;
				// Stomp: squash on landing.
				await monkey.animate(
					[
						{ transform: `translate(${to.x}px, ${to.y}px) translate(-50%, -50%) scale(1.25, 0.75)` },
						{ transform: `translate(${to.x}px, ${to.y}px) translate(-50%, -50%) scale(1)` }
					],
					{ duration: 160, fill: 'forwards' }
				).finished;
				falls.push(await smash(card));
				await sleep(120);
			}
			if (cards.length === 0) {
				const to = { x: innerWidth / 2, y: innerHeight / 2 };
				await hop(monkey, at, to, 700);
				at = to;
				burst(to.x, to.y);
				await sleep(300);
			}
			await hop(monkey, at, start, 750);
		} finally {
			monkey.remove();
			host.style.visibility = '';
			host.animate([{ transform: 'scale(0.6)' }, { transform: 'scale(1.15)' }, { transform: 'scale(1)' }], { duration: 300, easing: 'ease-out' });
			await sleep(700);
			// Cards pop back in, one after another.
			for (const f of falls) {
				const card = f.effect instanceof KeyframeEffect ? (f.effect.target as HTMLElement) : null;
				f.cancel();
				card?.animate(
					[
						{ transform: 'scale(0.3)', opacity: 0 },
						{ transform: 'scale(1.06)', opacity: 1, offset: 0.7 },
						{ transform: 'scale(1)', opacity: 1 }
					],
					{ duration: 420, easing: 'ease-out' }
				);
				await sleep(120);
			}
			busy = false;
		}
	}
</script>

<button bind:this={host} class="mascot" type="button" onclick={rampage} aria-label="Poke the monkey" title="Poke me!">
	<!-- eslint-disable-next-line svelte/no-at-html-tags -- our own static asset -->
	{@html logoSvg}
</button>

<style>
	.mascot {
		display: inline-grid;
		width: 56px;
		height: 56px;
		padding: 0;
		border: 0;
		background: none;
		cursor: pointer;
		transition: transform 0.12s ease;
	}
	.mascot:hover {
		transform: rotate(-8deg) scale(1.08);
	}
	.mascot:focus-visible {
		outline: 4px solid var(--accent);
		outline-offset: 3px;
		border-radius: 50%;
	}
	.mascot :global(svg) {
		width: 100%;
		height: 100%;
		overflow: visible;
	}
	.mascot :global(svg title) {
		display: none;
	}
	.mascot :global(.pupil) {
		transform-box: view-box;
		transition: transform 0.08s linear;
	}
	:global(.egg-monkey) {
		position: fixed;
		left: 0;
		top: 0;
		width: 96px;
		height: 96px;
		z-index: 1000;
		pointer-events: none;
		filter: drop-shadow(0 6px 6px rgba(34, 24, 79, 0.35));
	}
	:global(.egg-monkey svg) {
		width: 100%;
		height: 100%;
	}
	:global(.egg-block) {
		position: fixed;
		width: 16px;
		height: 16px;
		z-index: 999;
		pointer-events: none;
		border: 2px solid #22184f;
		border-radius: 3px;
	}
	:global(.egg-bonk) {
		position: fixed;
		z-index: 1001;
		pointer-events: none;
		font: 44px/1 var(--font-display);
		color: #ffd60a;
		-webkit-text-stroke: 2px #22184f;
		paint-order: stroke fill;
		text-shadow: 0 4px 0 #22184f;
		white-space: nowrap;
	}
	@media (prefers-reduced-motion: reduce) {
		.mascot,
		.mascot :global(.pupil) {
			transition: none;
		}
		.mascot:hover {
			transform: none;
		}
	}
</style>
