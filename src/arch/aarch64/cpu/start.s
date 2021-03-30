.globl _start
.extern LD_STACK_PTR
.extern __virt_start
.extern __load_start
.extern ttbr1_pa

.section ".text._start"

.equ PA_START,			0x0000000040080000
.equ VA_START, 			0xFFFF000000000000
.equ DEVICE_PA_BASE, 	0x0000000009000000
.equ DEVICE_VA_BASE, 	0x0000F000
.equ PHYS_MEMORY_SIZE, 	0x00001000

.equ PG_DIR_SIZE, 		512
.equ PAGE_SHIFT	 	,	12
.equ TABLE_SHIFT 	,	9
.equ SECTION_SHIFT	,	(PAGE_SHIFT + TABLE_SHIFT)
.equ PAGE_SIZE   	,	(1 << PAGE_SHIFT)	
.equ SECTION_SIZE	,	(1 << SECTION_SHIFT)	

.equ PTRS_PER_TABLE	,	(1 << TABLE_SHIFT)
.equ PGD_SHIFT		,	PAGE_SHIFT + 3*TABLE_SHIFT
.equ PUD_SHIFT		,	PAGE_SHIFT + 2*TABLE_SHIFT
.equ PMD_SHIFT		,	PAGE_SHIFT + TABLE_SHIFT
.equ PG_DIR_SIZE	,	(3 * PAGE_SIZE)

.equ MM_TYPE_PAGE_TABLE		, 0x3
.equ MM_TYPE_PAGE 			, 0x3
.equ MM_TYPE_BLOCK			, 0x1
.equ MM_ACCESS				, (0x1 << 10)
.equ MM_ACCESS_PERMISSION	, (0x01 << 6) 

.equ MT_DEVICE_nGnRnE 		,	0x0
.equ MT_NORMAL_NC			,	0x1
.equ MT_DEVICE_nGnRnE_FLAGS	,	0x00
.equ MT_NORMAL_NC_FLAGS  	,	0x44

.equ MMU_FLAGS	 			, (MM_TYPE_BLOCK | (MT_NORMAL_NC << 2) | MM_ACCESS)	
.equ MMU_DEVICE_FLAGS		, (MM_TYPE_BLOCK | (MT_DEVICE_nGnRnE << 2) | MM_ACCESS)	
.equ SCTLR_MMU_ENABLED      , (1 << 0)

.equ MAIR_VALUE				,	(MT_DEVICE_nGnRnE_FLAGS << (8 * MT_DEVICE_nGnRnE)) | (MT_NORMAL_NC_FLAGS << (8 * MT_NORMAL_NC))

.equ TCR_T0SZ	,		(64 - 48) 
.equ TCR_T1SZ	,		((64 - 48) << 16)
.equ TCR_TG0_4K	,		(0 << 14)
.equ TCR_TG1_4K	,		(2 << 30)
.equ TCR_VALUE	,		(TCR_T0SZ | TCR_T1SZ | TCR_TG0_4K | TCR_TG1_4K)

_start:
	//     ldr     x30, =LD_STACK_PTR
	//     mov     sp, x30
	//     bl      start
	mrs	x0, mpidr_el1		
	and	x0, x0,#0xFF		// Check processor id
	cbz	x0, el1_entry		// Hang for all non-primary CPU
	b	proc_hang

proc_hang: 
	wfe
	b proc_hang				

el1_entry:

	bl 	__create_page_tables

	// Calculte stack in phy addr
	ldr	x1, =__virt_start	
	ldr	x0, =LD_STACK_PTR
	sub x0, x0, x1	
	ldr	x1, =__load_start			
	add	sp, x0, x1

	adrp	x0, ttbr1_pa				
	msr	ttbr1_el1, x0

	ldr	x0, =(TCR_VALUE)		
	msr	tcr_el1, x0

	ldr	x0, =(MAIR_VALUE)
	msr	mair_el1, x0


	ldr	x2, =start

	mov	x0, #SCTLR_MMU_ENABLED				
	msr	sctlr_el1, x0

	isb
	br 	x2

	.macro	create_pgd_entry, tbl, virt, tmp1, tmp2
	create_table_entry \tbl, \virt, PGD_SHIFT, \tmp1, \tmp2
	create_table_entry \tbl, \virt, PUD_SHIFT, \tmp1, \tmp2
	.endm

	.macro	create_table_entry, tbl, virt, shift, tmp1, tmp2
	lsr	\tmp1, \virt, #\shift
	and	\tmp1, \tmp1, #PTRS_PER_TABLE - 1			// table index
	add	\tmp2, \tbl, #PAGE_SIZE
	orr	\tmp2, \tmp2, #MM_TYPE_PAGE_TABLE	
	str	\tmp2, [\tbl, \tmp1, lsl #3]
	add	\tbl, \tbl, #PAGE_SIZE					// next level table page
	.endm

	.macro	create_block_map, tbl, phys, start, end, flags, tmp1
	lsr	\start, \start, #SECTION_SHIFT
	and	\start, \start, #PTRS_PER_TABLE - 1			// table index
	lsr	\end, \end, #SECTION_SHIFT
	and	\end, \end, #PTRS_PER_TABLE - 1				// table end index
	lsr	\phys, \phys, #SECTION_SHIFT
	mov	\tmp1, #\flags
	orr	\phys, \tmp1, \phys, lsl #SECTION_SHIFT			// table entry
9999:	str	\phys, [\tbl, \start, lsl #3]				// store the entry
	add	\start, \start, #1					// next entry
	add	\phys, \phys, #SECTION_SIZE				// next block
	cmp	\start, \end
	b.ls	9999b
	.endm

__create_page_tables:
	mov	x29, x30						// save return address

	adrp	x0, ttbr1_pa
	mov	x1, #PG_DIR_SIZE
	bl 	__memzero

	adrp	x0, ttbr1_pa
	mov	x1, #VA_START 
	create_pgd_entry x0, x1, x2, x3

	/* Mapping kernel and init stack*/
	ldr x1, =PA_START							// start mapping from physical offset 0
	mov x2, #VA_START						// first virtual address
	ldr	x3, =(VA_START + DEVICE_VA_BASE - SECTION_SIZE)		// last virtual address
	create_block_map x0, x1, x2, x3, MMU_FLAGS, x4

	// /* Mapping device memory*/
	// mov x1, #DEVICE_PA_BASE					// start mapping from device base address 
	// ldr x2, =(VA_START + DEVICE_VA_BASE)				// first virtual address
	// ldr	x3, =(VA_START + PHYS_MEMORY_SIZE - SECTION_SIZE)	// last virtual address
	// create_block_map x0, x1, x2, x3, MMU_DEVICE_FLAGS, x4

	mov	x30, x29						// restore return address
	ret

__memzero:
	str xzr, [x0], #8
	subs x1, x1, #8
	b.gt __memzero
	ret

setup_init_mmu:
 	mov	x29, x30						// save return address

 	adrp x0, ttbr1_pa
 	mov	x1, #PG_DIR_SIZE
 	bl 	__memzero
   
 	adrp	x0, ttbr1_pa
 	mov	x1, #VA_START 

    lsr	x2, x1, #PGD_SHIFT // 0xFFFF000000000000 >> (12 + 3 * 9) = 0xFFFFFFFFFFFFFFFF
 	and	x2, x2, #PTRS_PER_TABLE - 1			// table index = 0
 	add	x3, x0, #PAGE_SIZE // ttbr1_pa + 4096 = 0x400a5000
 	orr	x3, x3, #MM_TYPE_PAGE_TABLE	
 	str	x3, [x0, x2, lsl #3] // to ttbr1_pa + (0 << 3)
 	add	x0, x0, #PAGE_SIZE
   
    lsr	x2, x1, #PUD_SHIFT // 0xFFFF000000000000 >> (12 + 2 * 9) = 0xFFFFFFFFFFFFFFFF
 	and	x2, x2, #PTRS_PER_TABLE - 1			// table index = 0
 	add	x3, x0, #PAGE_SIZE // ttbr1_pa + 4096 = 0x400a5000
 	orr	x3, x3, #MM_TYPE_PAGE_TABLE	
 	str	x3, [x0, x2, lsl #3] // to ttbr1_pa + (0 << 3)
 	add	x0, x0, #PAGE_SIZE		
	
	// SECTION_SHIFT = 9+12=21
	// PTRS_PER_TABLE = 512
	lsr	\start, \start, #SECTION_SHIFT               // 0xFFFF000000000000 >> (9+12) = 0xFFFFFFFFF8000000
	and	\start, \start, #PTRS_PER_TABLE - 1			// table index 0xFFFFFFFFF8000000 && 511 = 0
	lsr	\end, \end, #SECTION_SHIFT
	and	\end, \end, #PTRS_PER_TABLE - 1				// table end index
	lsr	\phys, \phys, #SECTION_SHIFT				// 0x0000000040080000 >> (9+12) = 0x200
	mov	\tmp1, #\flags								// 0x405
	orr	\phys, \tmp1, \phys, lsl #SECTION_SHIFT			// table entry // 0x405 | (0x200<<21)
9999:	str	\phys, [\tbl, \start, lsl #3]				// store the entry
	add	\start, \start, #1					// next entry
	add	\phys, \phys, #SECTION_SIZE				// next block
	cmp	\start, \end
	b.ls	9999b
   
 	mov	x30, x29						// restore return address
 	ret