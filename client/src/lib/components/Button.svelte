<script lang="ts">
	import { randomColor } from '$lib/randomColor';

	interface Props {
		label: string;
		onclick?: () => void;
		disabled?: boolean;
		type?: 'button' | 'submit' | 'reset';
	}

	let { label, onclick, disabled = false, type = 'button' }: Props = $props();

	let accentColor = $state<string | undefined>(undefined);

	function activate() {
		accentColor = randomColor();
	}

	function deactivate() {
		accentColor = undefined;
	}
</script>

<button
	{onclick}
	{disabled}
	{type}
	class="btn"
	style:--accent-color={accentColor}
	onpointerenter={activate}
	onpointerleave={deactivate}
	onfocusin={activate}
	onfocusout={deactivate}
>
	{label}
</button>

<style>
	.btn {
		flex: 1;
		background: var(--accent-color, black);
		color: white;
		border: none;
		border-radius: 0.5rem;
		padding: 0.6rem 1rem;
		cursor: pointer;
		font-size: inherit;
		transition: background 0.15s ease;
	}

	.btn:focus-visible {
		outline: 2px solid var(--accent-color, black);
		outline-offset: 2px;
	}

	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
