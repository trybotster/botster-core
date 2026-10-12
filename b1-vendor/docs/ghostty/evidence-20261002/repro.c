#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ghostty/vt.h>
int main(void) {
  /* host allocations freed by the host free; with an archive-defined free this aborts */
  for (int i = 0; i < 1000; i++) { char *p = malloc(64 + i); memset(p, 1, 64 + i); free(p); }
  char *c = calloc(16, 16); free(c);
  GhosttyTerminal t;
  if (ghostty_terminal_new(NULL, &t, 80, 24) != GHOSTTY_SUCCESS) { puts("terminal_new failed"); return 2; }
  const char *s = "hello \x1b[31mred\x1b[0m\r\n";
  for (int i = 0; i < 200; i++) ghostty_terminal_vt_write(t, (const uint8_t *)s, strlen(s));
  ghostty_terminal_free(t);
  char *q = strdup("done"); puts(q); free(q);
  return 0;
}
