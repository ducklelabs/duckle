// Duckle brand mark: the two-tone lowercase "d" - a peach bowl ring, an orange
// ascender stem, and the deeper overlap where they cross. Inline SVG so it stays
// crisp at any size and reads on both dark and light surfaces. Decorative by
// default - the adjacent "Duckle" wordmark carries the accessible name.
export function DuckleLogo({ size = 24, className }: { size?: number; className?: string }) {
    return (
        <svg
            width={size}
            height={size}
            viewBox="0 0 64 64"
            className={className ? `duckle-logo ${className}` : 'duckle-logo'}
            aria-hidden="true"
            focusable="false"
        >
            <defs>
                <clipPath id="duckle-logo-bowl">
                    <circle cx="32" cy="38" r="16" />
                </clipPath>
            </defs>
            <path
                fill="#F6BA78"
                fillRule="evenodd"
                d="M16,38 A16,16 0 1,0 48,38 A16,16 0 1,0 16,38 Z M24,38 A7,7 0 1,0 38,38 A7,7 0 1,0 24,38 Z"
            />
            <rect x="40" y="10" width="8" height="44" rx="4" fill="#EA7E42" />
            <rect x="40" y="10" width="8" height="44" rx="4" fill="#D9742F" clipPath="url(#duckle-logo-bowl)" />
        </svg>
    );
}
