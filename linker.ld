/* KPIO Kernel Linker Script */
/* x86_64 커널용 메모리 레이아웃 정의 */

ENTRY(kernel_main)

/* 커널 가상 주소 시작 (higher half) */
KERNEL_VADDR = 0xFFFF800000000000;

SECTIONS
{
    /* 커널은 higher half에 로드 */
    . = KERNEL_VADDR;

    /* 텍스트 섹션 (코드) */
    .text : AT(ADDR(.text) - KERNEL_VADDR)
    {
        _text_start = .;
        *(.text .text.*)
        _text_end = .;
    }

    /* 읽기 전용 데이터 */
    .rodata : AT(ADDR(.rodata) - KERNEL_VADDR)
    {
        _rodata_start = .;
        *(.rodata .rodata.*)
        _rodata_end = .;
    }

    /* 초기화된 데이터 */
    .data : AT(ADDR(.data) - KERNEL_VADDR)
    {
        _data_start = .;
        *(.data .data.*)
        _data_end = .;
    }

    /* BSS (제로 초기화 데이터) */
    .bss : AT(ADDR(.bss) - KERNEL_VADDR)
    {
        _bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        _bss_end = .;
    }

    /* 페이지 정렬 */
    . = ALIGN(4096);
    _kernel_end = .;

    /* bootloader_api 설정 섹션 - 부트로더가 이 섹션을 읽음 */
    .bootloader : AT(ADDR(.bootloader) - KERNEL_VADDR)
    {
        KEEP(*(.bootloader_config))
    }

    /* 디버그 섹션 제거 (릴리스 빌드) */
    /DISCARD/ :
    {
        *(.comment)
        *(.note.*)
        *(.eh_frame*)
    }
}
