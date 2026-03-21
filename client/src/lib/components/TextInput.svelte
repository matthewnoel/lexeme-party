<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	interface Props extends HTMLInputAttributes {
		value?: string;
	}

	let { value = $bindable(''), ...rest }: Props = $props();
</script>

<input bind:value class="input" {...rest} />

<style>
	.input {
		background: transparent;
		color: inherit;
		border: 1px solid var(--color-primary);
		border-radius: 0.5rem;
		padding: 0.6rem;
		font-size: inherit;
		transition: border-color 0.15s ease;
	}

	.input:hover {
		border-color: var(--color-secondary);
	}

	.input:focus-visible {
		outline: 2px solid var(--color-secondary);
		outline-offset: 2px;
	}
</style>
