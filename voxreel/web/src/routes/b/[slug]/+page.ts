import { error } from '@sveltejs/kit';
import { battles, getBattle } from '#lib/builds.ts';
import type { EntryGenerator, PageLoad } from './$types';

export const entries: EntryGenerator = () => battles.map((b) => ({ slug: b.slug }));

export const load: PageLoad = ({ params }) => {
	const battle = getBattle(params.slug);
	if (!battle) error(404, 'No such battle');
	return { battle, others: battles.filter((b) => b.slug !== params.slug) };
};
