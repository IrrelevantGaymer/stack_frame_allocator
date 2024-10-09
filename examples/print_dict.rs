use stack_frame_allocators::stack_frame_dict_allocator::StackFrameDictAllocator;

pub fn main() {
    let stack = StackFrameDictAllocator::<&str, usize>::new();
    stack.push("I", 1);
    stack.print();
    stack.push("II", 2);
    stack.print();
    stack.push("III", 3);
    stack.print();

    stack.new_scope(|stack| {
        stack.push("a", 10);
        stack.print();
        stack.push("b", 20);
        stack.print();

        stack.new_scope(|stack| {
            stack.push("1", 100);
            stack.print();
            stack.push("2", 200);
            stack.print();
            stack.push("3", 300);
            stack.print();
            stack.push("4", 400);
            stack.print();
            stack.push("5", 500);
            stack.print();
        });

        stack.push("c", 30);
        stack.print();
    });

    stack.push("IV", 4);
    stack.print();
    stack.push("V", 5);
    stack.print();
    stack.push("VI", 6);
    stack.print();
}