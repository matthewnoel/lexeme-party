<script lang="ts">
	import type { HTMLSelectAttributes } from 'svelte/elements';

	interface SelectOption {
		value: string;
		label: string;
	}

	interface Props extends HTMLSelectAttributes {
		value?: string;
		options: SelectOption[];
	}

	let { value = $bindable(''), options, ...rest }: Props = $props();
</script>

<select bind:value class="select" {...rest}>
	{#each options as opt (opt.value)}
		<option value={opt.value}>{opt.label}</option>
	{/each}
</select>

<style>
	.select {
		appearance: none;
		background: transparent;
		color: inherit;
		border: 1px solid var(--color-primary);
		border-radius: 0.5rem;
		padding: 0.6rem;
		padding-right: 2rem;
		font-size: inherit;
		transition: border-color 0.15s ease;
		cursor: pointer;
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' fill='none'%3E%3Cpath d='M1 1.5l5 5 5-5' stroke='%23999' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
		background-repeat: no-repeat;
		background-position: right 0.6rem center;
	}

	.select:hover {
		border-color: var(--color-secondary);
	}

	.select:focus-visible {
		outline: 2px solid var(--color-secondary);
		outline-offset: 2px;
	}
</style>
