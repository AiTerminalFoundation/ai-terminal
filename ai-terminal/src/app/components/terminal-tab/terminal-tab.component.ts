import { CommonModule } from '@angular/common';
import { Component, EventEmitter, Input, Output } from '@angular/core';
import { TerminalSession } from '../../models/terminal-session.model';

@Component({
  selector: 'app-terminal-tab',
  imports: [CommonModule],
  templateUrl: './terminal-tab.component.html',
  styleUrl: './terminal-tab.component.css'
})
export class TerminalTabComponent {
  @Input() session!: TerminalSession;
  @Input() canClose: boolean = false;

  @Output() select = new EventEmitter<string>();
  @Output() close = new EventEmitter<string>();
  @Output() renameStart = new EventEmitter<Event>();
  @Output() renameFinish = new EventEmitter<{ event: Event; session: TerminalSession }>();
  @Output() renameEnter = new EventEmitter<Event>();

  onSelect(): void {
    this.select.emit(this.session.id);
  }

  onClose(event: Event): void {
    event.stopPropagation();
    this.close.emit(this.session.id);
  }

  onRenameStart(event: Event): void {
    this.renameStart.emit(event);
  }

  onRenameFinish(event: Event): void {
    this.renameFinish.emit({ event, session: this.session });
  }

  onRenameEnter(event: Event): void {
    this.renameEnter.emit(event);
  }
}
