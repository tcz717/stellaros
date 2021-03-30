__load_start = 0x0000000040080000;

SECTIONS
{
    /* Set current address to the value from which the RPi starts execution */
    . = __load_start;
    __ro_start = .;
    .stext :
    {
        *(.text._start) 
    }

    .text :
    {
        *(.text*)
    }

    .rodata : 
    {
        *(.rodata*)
    }
    . = ALIGN(65536); /* Fill up to 64 KiB */
    __ro_end = .;

    .data : 
    {
        *(.data*)
    }

    /* Section is zeroed in u64 chunks, align start and end to 8 bytes */
    .bss ALIGN(8): 
    {
        __bss_start = .;
        *(.bss*);
        . = ALIGN(8);

        /* Fill for the bss == 0 case, so that __bss_start <= __bss_end_inclusive holds */
        . += 8;
        __bss_end_inclusive = . - 8;
    }
    . = ALIGN(65536);
    __data_end = .;
    
    . = ALIGN(8);
    . = . + 0x4000;
    LD_STACK_PTR = .;
    
    __load_end = .;
    __load_size = __load_end - __load_start;

    __ro_size = __ro_end - __ro_start;
    __data_size = __data_end - __ro_end;

    /DISCARD/ : { *(.comment*) }
}
