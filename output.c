#include <stdio.h>
#include <stdint.h>

int user_main() {
    printf("%lld\n", 42LL);
    printf("%lld\n", 100LL);
    return 0;
}

int main() {
    return user_main();
}
