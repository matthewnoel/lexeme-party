<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import { randomColor } from '$lib/randomColor';

	interface Props extends HTMLInputAttributes {
		value?: string;
	}

	let { value = $bindable(''), ...rest }: Props = $props();

	let accentColor = $state<string | undefined>(undefined);

	function activate() {
		accentColor ??= randomColor();
	}

	function deactivate() {
		accentColor = undefined;
	}
</script>

<input
	bind:value
	class="input"
	style:--accent-color={accentColor}
	onpointerenter={activate}
	onpointerleave={deactivate}
	onfocusin={activate}
	onfocusout={deactivate}
	{...rest}
/>

<style>
	.input {
		flex: 1;
		background: transparent;
		color: inherit;
		border: 2px solid black;
		border-radius: 0.5rem;
		padding: 0.6rem;
		font-size: inherit;
		transition:
			border-color 0.15s ease,
			outline-color 0.15s ease;
	}

	.input:hover {
		border-color: var(--accent-color, black);
	}

	.input:focus-visible {
		outline: 2px solid var(--accent-color, black);
		outline-offset: 2px;
	}
</style>
