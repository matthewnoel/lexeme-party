<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { browser } from '$app/environment';
	import { onMount, onDestroy } from 'svelte';
	import {
		gs,
		connect,
		setOnDisconnect,
		handlePromptInput,
		submitPrompt,
		socketStateLabel,
		defaultWsUrl,
		loadSession,
		loadRejoinToken,
		disconnect
	} from '$lib/game/connection.svelte';
	import { nextBlobLayout, type BlobLayout } from '$lib/game/sim';
	import type { PlayerSnapshot } from '$lib/game/protocol';
	import { debugMode } from '$lib/debug';
	import Button from '$lib/components/Button.svelte';
	import TextInput from '$lib/components/TextInput.svelte';

	let arenaEl: HTMLDivElement | null = $state(null);
	let blobLayout: BlobLayout = $state({});
	let debugOpen = $state(false);
	let animationHandle = 0;
	let visualHeight = $state(0);

	$effect(() => {
		function update() {
			visualHeight = window.visualViewport?.height ?? window.innerHeight;
		}
		update();
		window.visualViewport?.addEventListener('resize', update);
		return () => window.visualViewport?.removeEventListener('resize', update);
	});

	function animate(): void {
		if (gs.room && arenaEl) {
			blobLayout = nextBlobLayout(
				gs.room.players,
				blobLayout,
				performance.now(),
				arenaEl.clientWidth,
				arenaEl.clientHeight
			);
		}
		animationHandle = requestAnimationFrame(animate);
	}

	function circleSize(player: PlayerSnapshot): number {
		return Math.max(42, Math.min(220, player.size * 4));
	}

	function leaveRoom(): void {
		disconnect();
		goto(resolve('/'));
	}

	function copyRoomLink(): void {
		navigator.clipboard.writeText(window.location.href);
	}

	onMount(() => {
		setOnDisconnect(() => goto(resolve('/')));

		if (gs.phase !== 'ingame') {
			const code = page.params.code ?? '';
			const session = loadSession();
			const rejoinToken = loadRejoinToken(code);
			connect(session?.wsUrl ?? defaultWsUrl(), {
				roomCode: code,
				playerName: session?.playerName,
				gameMode: session?.gameMode,
				rejoinToken: rejoinToken ?? undefined
			});
		}

		animationHandle = requestAnimationFrame(animate);
	});

	onDestroy(() => {
		if (browser) {
			cancelAnimationFrame(animationHandle);
		}
		setOnDisconnect(null);
	});
</script>

<main class="game" style:--vvh={visualHeight ? `${visualHeight}px` : null}>
	<div class="leave">
		<Button label="Leave" onclick={leaveRoom} />
	</div>
	<header>
		<div class="prompt"><strong>{gs.room?.prompt ?? 'Waiting for prompt...'}</strong></div>
		<div class="input-container">
			<TextInput
				value={gs.promptInput}
				oninput={(e) => handlePromptInput(e.currentTarget.value)}
				onkeydown={(e) => {
					if (e.key === 'Enter') submitPrompt();
				}}
				placeholder="Type your answer, press Enter to submit"
				autocomplete="off"
				autocorrect="off"
				autocapitalize="off"
				spellcheck="false"
			/>
		</div>
		{#if gs.latestRoundSummary}
			<div class="result" style:color={gs.latestRoundSummaryColor || null}>
				{gs.latestRoundSummary}
			</div>
		{/if}
	</header>
	<div class="arena" bind:this={arenaEl}>
		{#if gs.room}
			{#each gs.room.players as player (player.id)}
				<div
					class="blob {player.id === gs.playerId ? 'me' : ''}"
					style={`--blob-color:${player.color}; width:${circleSize(player)}px; height:${circleSize(player)}px; left:${(blobLayout[player.id]?.x ?? 0) - circleSize(player) / 2}px; top:${(blobLayout[player.id]?.y ?? 0) - circleSize(player) / 2}px;`}
				>
					<div class="name">{player.name}</div>
					<div class="size">{player.size.toFixed(1)}</div>
					<div class="progress">{player.progress}</div>
				</div>
			{/each}
		{/if}
	</div>
	{#if gs.room?.roomCode}
		<div class="room">
			<input
				type="button"
				class="shizuru-regular"
				value={gs.room.roomCode}
				onclick={copyRoomLink}
			/>
		</div>
	{/if}
	{#if debugMode}
		<aside class="debug">
			<Button
				label={debugOpen ? 'Hide' : 'Stats for nerds'}
				onclick={() => (debugOpen = !debugOpen)}
			/>
			{#if debugOpen}
				<dl>
					<dt>game</dt>
					<dd>{gs.gameKey || 'unknown'}</dd>
					<dt>room</dt>
					<dd>{gs.room?.roomCode ?? '-'}</dd>
					<dt>socket</dt>
					<dd>{socketStateLabel()}</dd>
					<dt>inbound</dt>
					<dd>{gs.inboundCount}</dd>
					<dt>outbound</dt>
					<dd>{gs.outboundCount}</dd>
					<dt>players</dt>
					<dd>{gs.room?.players.length ?? 0}</dd>
					<dt>min eat size</dt>
					<dd>{gs.minEatableSize.toFixed(1)}</dd>
				</dl>
			{/if}
		</aside>
	{/if}
</main>

<style>
	main {
		min-height: 100vh;
	}

	.game {
		display: grid;
		grid-template-rows: auto 1fr;
	}

	header {
		padding: 0.75rem;
		display: grid;
		gap: 0.5rem;
		position: relative;
		z-index: 2;
	}

	.prompt {
		font-size: 2rem;
		text-align: center;
		margin: 4rem 0 2rem 0;
	}

	.input-container {
		display: flex;
		margin: 0 auto;
		width: 100%;
		max-width: 400px;
	}

	.result {
		font-size: 0.9rem;
		text-align: center;
		margin-top: 0.25rem;
	}

	.arena {
		position: relative;
		overflow: hidden;
		min-height: 62vh;
	}

	.blob {
		position: absolute;
		background: var(--blob-color);
		border: 2px solid var(--blob-color);
		border-radius: 9999px;
		display: grid;
		place-content: center;
		gap: 0.2rem;
		text-align: center;
		padding: 0.5rem;
		box-sizing: border-box;
		transition:
			width 180ms linear,
			height 180ms linear;
	}

	.blob.me {
		outline: 2px solid;
	}

	.name {
		font-size: 0.85rem;
		font-weight: 600;
	}

	.size,
	.progress {
		font-size: 0.75rem;
	}

	.leave {
		position: fixed;
		top: 0.5rem;
		left: 0.5rem;
		right: 0.5rem;
		z-index: 3;
	}

	.room {
		position: fixed;
		bottom: 2rem;
		left: 0.5rem;
		right: 0.5rem;
		z-index: 3;
		text-align: center;
	}

	.room input[type='button'] {
		background-color: transparent;
		border: none;
		color: black;
		font-size: 3rem;
		cursor: pointer;
	}

	.debug {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		position: fixed;
		right: 0.5rem;
		bottom: 0.5rem;
		border-radius: 0.4rem;
		padding: 0.5rem;
		width: 240px;
		z-index: 3;
	}

	dl {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.15rem 0.4rem;
		margin: 0.45rem 0 0 0;
		font-size: 0.8rem;
	}

	dd {
		margin: 0;
		text-align: right;
	}

	@media (max-width: 768px) and (orientation: portrait) {
		main {
			min-height: 0;
			height: var(--vvh, 100vh);
			max-height: var(--vvh, 100vh);
			overflow: hidden;
		}

		.arena {
			min-height: 0;
		}
	}
</style>
