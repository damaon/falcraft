// Same palettes as the video renderer (src/render.rs `Theme::by_name`).
export type ThemeName = 'day' | 'candy' | 'sunset' | 'night' | 'moon' | 'mint';

export interface Theme {
	skyTop: string;
	skyHorizon: string;
	ground: string;
	ground2: string;
	plate: string;
	night: boolean;
}

export const THEMES: Record<ThemeName, Theme> = {
	day: { skyTop: '#2478ec', skyHorizon: '#b2deff', ground: '#84ce66', ground2: '#78c25c', plate: '#fafafc', night: false },
	candy: { skyTop: '#9660eb', skyHorizon: '#ffc4e4', ground: '#ffdeec', ground2: '#ffcce2', plate: '#ffffff', night: false },
	sunset: { skyTop: '#4046af', skyHorizon: '#ffa870', ground: '#ecbea0', ground2: '#e2b094', plate: '#fff4ec', night: false },
	night: { skyTop: '#080a26', skyHorizon: '#403276', ground: '#2c3052', ground2: '#262a4a', plate: '#464a6e', night: true },
	moon: { skyTop: '#040410', skyHorizon: '#1a163c', ground: '#9696a2', ground2: '#8a8a96', plate: '#464656', night: true },
	mint: { skyTop: '#46bec8', skyHorizon: '#d6f8ec', ground: '#c4ecd6', ground2: '#b8e4cc', plate: '#ffffff', night: false }
};
