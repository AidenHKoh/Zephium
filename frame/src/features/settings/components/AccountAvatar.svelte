<script lang="ts">
  /** What an avatar shows. Only initials today; a photo joins this view. */
  let { view, size = 32 }: { view: { initials: string }; size?: number } = $props();
</script>

<span class="avatar" style:--avatar-size={`${size}px`} aria-hidden="true">{view.initials}</span>

<style>
  .avatar {
    display: grid;
    flex: none;
    inline-size: var(--avatar-size);
    block-size: var(--avatar-size);
    border-radius: var(--radius-capsule);
    background: var(--color-fill-strong);
    box-shadow: inset 0 0 0 0.5px var(--color-border);
    color: var(--color-text);
    font-size: calc(var(--avatar-size) * 0.4);
    font-weight: 550;
    letter-spacing: 0.01em;
    place-items: center;
  }
</style>
