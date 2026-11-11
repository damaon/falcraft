// Battles are read straight from voxreel/builds/<slug>/<n>.json at build time, so adding a
// folder there is all it takes to publish a new battle.
import type { BuildSpec } from './voxels';
import type { ThemeName } from './themes';

const files = import.meta.glob<BuildSpec>('../../../builds/*/*.json', { eager: true, import: 'default' });

export interface Battle {
	slug: string;
	title: string;
	emoji: string;
	theme: ThemeName;
	tagline: string;
	builds: BuildSpec[];
}

/** Presentation per battle; unknown folders fall back to a title derived from the slug. */
const META: Record<string, Omit<Battle, 'slug' | 'builds'>> = {
	'pirate-ship': { title: 'Pirate Ship', emoji: '🏴‍☠️', theme: 'day', tagline: 'A giant galleon, a cute boat or a treasure island?' },
	'candy-castle': { title: 'Candy Castle', emoji: '🍭', theme: 'candy', tagline: 'Which sweet house would you live in?' },
	rocket: { title: 'Rocket Ship', emoji: '🚀', theme: 'night', tagline: 'Which rocket gets to space first?' }
};

function titleCase(slug: string) {
	return slug.replace(/[-_]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

function collect(): Battle[] {
	const bySlug = new Map<string, [string, BuildSpec][]>();
	for (const [path, spec] of Object.entries(files)) {
		const [slug, file] = path.split('/').slice(-2);
		if (!bySlug.has(slug)) bySlug.set(slug, []);
		bySlug.get(slug)!.push([file, spec]);
	}
	return [...bySlug.entries()]
		.map(([slug, entries]) => {
			entries.sort(([a], [b]) => a.localeCompare(b, undefined, { numeric: true }));
			const meta = META[slug] ?? { title: titleCase(slug), emoji: '🧱', theme: 'day' as ThemeName, tagline: 'Which one is the best?' };
			return { slug, ...meta, builds: entries.map(([, s]) => s) };
		})
		.sort((a, b) => a.title.localeCompare(b.title));
}

export const battles = collect();

export function getBattle(slug: string) {
	return battles.find((b) => b.slug === slug);
}
