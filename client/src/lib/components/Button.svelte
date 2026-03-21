<script lang="ts">
	interface Props {
		label: string;
		onclick?: () => void;
		disabled?: boolean;
		type?: 'button' | 'submit' | 'reset';
	}

	let { label, onclick, disabled = false, type = 'button' }: Props = $props();
</script>

<button {onclick} {disabled} {type} class="btn">
	{label}
</button>

<style>
	.btn {
		flex: 1;
		background: var(--color-secondary);
		color: white;
		border: none;
		border-radius: 0.5rem;
		padding: 0.6rem 1rem;
		cursor: pointer;
		font-size: inherit;
		transition: opacity 0.15s ease;
	}

	.btn:hover:not(:disabled) {
		opacity: 0.85;
	}

	.btn:focus-visible {
		outline: 2px solid var(--color-secondary);
		outline-offset: 2px;
	}

	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
