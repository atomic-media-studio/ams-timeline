use egui::Rect;
use std::cell::Cell;

/// A context for instantiating tracks, either pinned or unpinned.
pub struct TracksCtx {
    /// The rectangle encompassing the entire widget area including both header and timeline and
    /// both pinned and unpinned track areas.
    pub full_rect: Rect,
    /// The rect encompassing the left-hand-side track headers including pinned and unpinned.
    pub header_full_rect: Option<Rect>,
    /// Context specific to the timeline (non-header) area.
    pub timeline: TimelineCtx,
    /// Counter for pinned tracks (0 = Ruler, 1 = Marker, etc.)
    pinned_track_index: Cell<usize>,
}

/// Some context for the timeline, providing short-hand for setting some useful widgets.
pub struct TimelineCtx {
    /// The total visible rect of the timeline area including pinned and unpinned tracks.
    pub full_rect: Rect,
    /// The total number of ticks visible on the timeline area.
    pub visible_ticks: f32,
}

/// A type used to assist with setting a track with an optional `header`.
pub struct TrackCtx<'a> {
    tracks: &'a TracksCtx,
    ui: &'a mut egui::Ui,
    available_rect: Rect,
    header_height: f32,
    track_id: Option<String>,
    is_first_track: bool,
    is_last_track: bool,
    pinned_track_index: Option<usize>, // None for regular tracks, Some(index) for pinned tracks
}

/// Context for instantiating the playhead after all tracks have been set.
pub struct SetPlayhead {
    timeline_rect: Rect,
    /// The y position at the top of the first track (after ruler + spacing).
    tracks_top: f32,
    /// The y position at the bottom of the last track, or the bottom of the
    /// tracks' scrollable area in the case that the size of the tracks
    /// exceed the visible height.
    tracks_bottom: f32,
    /// The bottom bar rectangle (20px height at the bottom).
    pub(crate) bottom_bar_rect: Option<Rect>,
    /// The top panel rectangle (40px height at the top).
    pub(crate) top_panel_rect: Option<Rect>,
}

/// Relevant information for displaying a background for the timeline.
pub struct BackgroundCtx<'a> {
    pub header_full_rect: Option<Rect>,
    pub timeline: &'a TimelineCtx,
}

impl TracksCtx {
    /// Begin showing the next `Track`.
    pub fn next<'a>(&'a self, ui: &'a mut egui::Ui) -> TrackCtx<'a> {
        let available_rect = ui.available_rect_before_wrap();
        // Only assign pinned_track_index for tracks that are actually pinned (at the very top, y < 100px)
        // This prevents regular tracks from getting a pinned_track_index even if they're near the top
        // Regular tracks will call with_id() which sets track_id, so they'll use 4px spacing regardless
        let pinned_track_index = if available_rect.min.y < 100.0 {
            // Likely a pinned track - get and increment the index
            Some(self.next_pinned_track_index())
        } else {
            // Regular track - don't increment the counter
            None
        };
        TrackCtx {
            tracks: self,
            ui,
            available_rect,
            header_height: 0.0,
            track_id: None,
            is_first_track: false,
            is_last_track: false,
            pinned_track_index,
        }
    }
    
}

impl<'a> TrackCtx<'a> {
    /// Set the track identifier for selection tracking.
    pub fn with_id(mut self, track_id: impl Into<String>) -> Self {
        self.track_id = Some(track_id.into());
        self
    }
    
    /// Mark this track as the first regular track (after ruler)
    pub fn mark_first_track(mut self) -> Self {
        self.is_first_track = true;
        self
    }
    
    /// Mark this track as the last regular track
    pub fn mark_last_track(mut self) -> Self {
        self.is_last_track = true;
        self
    }

    /// UI for the track's header.
    ///
    /// The header content (text, buttons, etc.) is automatically padded 4px from the left edge
    /// to provide consistent spacing for track labels and controls like mute/solo buttons.
    ///
    /// NOTE: Both the ruler (pinned track) and regular tracks use the same `header_full_rect`
    /// from `TracksCtx`, ensuring they always have the same width. The border is drawn at
    /// `header_full_rect.max.x` for both, guaranteeing alignment.
    pub fn header(mut self, header: impl FnOnce(&mut egui::Ui)) -> Self {
        const LEFT_PADDING: f32 = 4.0;
        let header_h = self
            .tracks
            .header_full_rect
            .map(|mut rect| {
                // IMPORTANT: Both ruler and tracks use the same header_full_rect, so they have the same width
                // The rect.max.x (header_right_x) is the same for both, ensuring the grey border aligns perfectly
                
                // Store original header rect boundaries before modifying rect
                // This ensures the border is drawn at the same x position for both ruler and tracks
                let header_right_x = rect.max.x; // Right edge of header (where grey border will be drawn)
                
                // Header starts at the available rect's top (with offset for first track)
                let track_offset_y = if self.is_first_track { 10.0 } else { 0.0 };
                let header_start_y = self.available_rect.min.y + track_offset_y;
                rect.min.y = header_start_y;
                // Constrain header height to available rect to prevent overlap with next track
                rect.max.y = rect.min.y.min(self.available_rect.max.y);
                
                // Fill the header area with background FIRST (before content) to prevent grid lines from showing through
                // This ensures the background is behind the widgets
                let vis = self.ui.style().noninteractive();
                
                // FIX: For the ruler (pinned track), constrain header background to only the header area
                // The ruler's header should only cover its own header, not extend into the track content or beyond
                // Use a reasonable fixed height estimate (20px matches ruler content height: 4px padding + ~16px label)
                const RULER_HEADER_HEIGHT_ESTIMATE: f32 = 20.0; // Matches ruler content height (RULER_HEIGHT)
                // For regular tracks, estimate header height (input field + buttons + padding ≈ 28-32px)
                const TRACK_HEADER_HEIGHT_ESTIMATE: f32 = 30.0; // Typical height for track name input + S/M buttons + padding
                let header_fill_max_y = if self.track_id.is_none() {
                    // Ruler: only cover the header area itself - use fixed estimate
                    // This ensures it stops at the ruler's header bottom, not extending into timeline content
                    header_start_y + RULER_HEADER_HEIGHT_ESTIMATE
                } else {
                    // Regular tracks: only cover the header area itself - use fixed estimate
                    // This ensures it stops at the track's header bottom, not extending into timeline content
                    // This prevents the shadow effect on the grey border from background extending too far down
                    header_start_y + TRACK_HEADER_HEIGHT_ESTIMATE
                };
                
                // FIX: Header background should extend to the right border (header_right_x) before grid lines start
                // This ensures the gray rectangle goes all the way to the grey vertical border
                let header_fill_rect = egui::Rect::from_min_max(
                    egui::Pos2::new(rect.min.x, header_start_y),
                    egui::Pos2::new(header_right_x, header_fill_max_y),
                );
                
                self.ui.painter().rect_filled(header_fill_rect, 0.0, vis.bg_fill);
                
                // Add 4px left padding by adjusting the rect
                rect.min.x += LEFT_PADDING;
                let ui = &mut self.ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(*self.ui.layout()),
                );
                header(ui);
                ui.min_rect().height()
            })
            .unwrap_or(0.0);
        self.header_height = header_h;
        self
    }

    /// Set the track, with a function for instantiating contents for the timeline.
    /// `on_track_click` is called when the full track area (header + content) is clicked.
    pub fn show(
        self,
        track: impl FnOnce(&TimelineCtx, &mut egui::Ui),
        playhead_api: Option<&dyn crate::playhead::PlayheadApi>,
        selection_api: Option<&dyn crate::interaction::TrackSelectionApi>,
        on_track_click: Option<impl FnOnce(String)>,
        is_selected: bool,
    ) {
        // For first track, add 10px offset for testing
        let track_offset_y = if self.is_first_track { 10.0 } else { 0.0 };
        
        // The UI and area for the track timeline.
        let track_timeline_rect = {
            let mut rect = self.tracks.timeline.full_rect;
            rect.min.y = self.available_rect.min.y + track_offset_y;
            rect
        };
        
        // Draw selection overlay BEFORE track content so blocks appear on top (higher z-order)
        // Use estimated full track rect - overlay will cover full area, blocks drawn later will appear on top
        if is_selected {
            let selection_overlay = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 5);
            // Estimate full track height (header + minimum content height)
            let estimated_track_h = 40.0; // Minimum track height
            let estimated_full_track_height = self.header_height.max(estimated_track_h);
            let estimated_full_track_rect = egui::Rect::from_min_max(
                egui::Pos2::new(
                    self.tracks.full_rect.min.x, // Left edge (includes header)
                    self.available_rect.min.y + track_offset_y,    // Top of this track (with offset)
                ),
                egui::Pos2::new(
                    self.tracks.full_rect.max.x,              // Right edge (full width)
                    self.available_rect.min.y + track_offset_y + estimated_full_track_height, // Bottom of this track
                ),
            );
            self.ui.painter().rect_filled(estimated_full_track_rect, 0.0, selection_overlay);
        }
        
        let track_h = {
            let ui = &mut self.ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(track_timeline_rect)
                    .layout(*self.ui.layout()),
            );
            track(&self.tracks.timeline, ui);
            ui.min_rect().height()
        };
        
        // Calculate the actual track area (only the height of this track, not the full timeline)
        let actual_track_rect = {
            let mut rect = track_timeline_rect;
            rect.max.y = track_timeline_rect.min.y + track_h;
            rect
        };
        
        // Calculate the full track rect (header + timeline, 100% width) - calculate once and reuse
        // This rect is ONLY for the track content, NOT including spacing
        let full_track_height = self.header_height.max(track_h);
        let full_track_rect = egui::Rect::from_min_max(
            egui::Pos2::new(
                self.tracks.full_rect.min.x, // Left edge (includes header)
                self.available_rect.min.y + track_offset_y,    // Top of this track (with offset for first track)
            ),
            egui::Pos2::new(
                self.tracks.full_rect.max.x,              // Right edge (full width)
                self.available_rect.min.y + track_offset_y + full_track_height, // Bottom of this track (NOT including spacing)
            ),
        );
        
        // Handle interaction for this track
        if let Some(track_id) = &self.track_id {
            // Get selection data before calling handle_track_interaction (which takes ownership)
            // Check if this track has the selection (only one selection exists across all tracks)
            let selection_data = selection_api.as_ref().and_then(|api| {
                if api.get_selected_track_id().as_ref() == Some(track_id) {
                    api.get_selection(track_id)
                } else {
                    None
                }
            });
            let ticks_per_point_for_selection = selection_api.as_ref().map(|api| api.ticks_per_point());
            
            crate::interaction::handle_track_interaction(
                self.ui,
                actual_track_rect,
                track_timeline_rect, // Pass full timeline rect for tick calculation
                track_id,
                playhead_api,
                selection_api,
            );
            
            // Draw selection if it exists on this track (now that we have full_track_rect)
            if let (Some((absolute_start_tick, absolute_end_tick)), Some(ticks_per_point)) = (selection_data, ticks_per_point_for_selection) {
                let timeline_w = track_timeline_rect.width();
                let visible_ticks = ticks_per_point * timeline_w;
                let timeline_start = selection_api.as_ref().map(|api| api.timeline_start()).unwrap_or(0.0);
                
                // Convert absolute ticks to relative ticks for drawing
                let relative_start_tick = absolute_start_tick - timeline_start;
                let relative_end_tick = absolute_end_tick - timeline_start;
                
                // Only draw if selection is visible in current viewport
                if relative_end_tick >= 0.0 && relative_start_tick <= visible_ticks {
                    let start_x = track_timeline_rect.min.x
                        + (relative_start_tick.max(0.0) / visible_ticks) * timeline_w;
                    let end_x = track_timeline_rect.min.x
                        + (relative_end_tick.min(visible_ticks) / visible_ticks) * timeline_w;
                    
                    // Selection should match exactly from top border to bottom border
                    // Top border is at full_track_rect.min.y, bottom border is at full_track_rect.max.y
                    // But selection only spans the timeline area (not header), so use timeline x coordinates
                    // Selection must never overflow the track bounds - clamp to track rect
                    let selection_top = full_track_rect.min.y; // Always start at track top, never above
                    let selection_bottom = full_track_rect.max.y; // Always end at track bottom, never below
                    let selection_rect = egui::Rect::from_min_max(
                        egui::Pos2::new(start_x.min(end_x), selection_top),
                        egui::Pos2::new(start_x.max(end_x), selection_bottom),
                    );
                    
                    // Draw selection normally (no special case for first track)
                    let selection_fill = egui::Color32::from_rgba_unmultiplied(100, 150, 255, 100);
                    self.ui.painter().rect_filled(selection_rect, 0.0, selection_fill);
                }
            }
        }
        
        // Handle track selection click (on full track area, 100% width and height)
        if let Some(track_id) = &self.track_id {
            if let Some(on_click) = on_track_click {
                // Check if pointer clicked on the full track area
                let pointer_pos = self.ui.input(|i| i.pointer.interact_pos());
                let pointer_pressed = self.ui.input(|i| i.pointer.primary_pressed());
                
                if pointer_pressed {
                    if let Some(pos) = pointer_pos {
                        if full_track_rect.contains(pos) {
                            // Select track on any click within the full track area (header + content)
                            // This includes the input string area and the timeline content area
                            on_click(track_id.clone());
                        }
                    }
                }
            }
        }
        
        // Draw a pink border around the track ONLY (not including spacing)
        // For the ruler (track_id is None), draw full border. For regular tracks, draw borders around track content only.
        let pink_border = egui::Stroke {
            width: 1.0,
            color: egui::Color32::from_rgb(255, 192, 203), // Pink
        };
        
        if self.track_id.is_none() {
            // Pinned tracks: draw complete border (all 4 sides) for both Ruler and Marker
            let left_top = egui::Pos2::new(full_track_rect.min.x, full_track_rect.min.y);
            let right_top = egui::Pos2::new(full_track_rect.max.x, full_track_rect.min.y);
            let left_bottom = egui::Pos2::new(full_track_rect.min.x, full_track_rect.max.y);
            let right_bottom = egui::Pos2::new(full_track_rect.max.x, full_track_rect.max.y);
            
            // Top border
            self.ui.painter().line_segment([left_top, right_top], pink_border);
            // Left border (pink border at left edge)
            self.ui.painter().line_segment([left_top, left_bottom], pink_border);
            // Right border
            self.ui.painter().line_segment([right_top, right_bottom], pink_border);
            // Bottom border
            self.ui.painter().line_segment([left_bottom, right_bottom], pink_border);
            
            // Draw left grey border for the ruler header area to match the header's right border position
            if let Some(header_rect) = self.tracks.header_full_rect {
                let header_right_x = header_rect.max.x;
                let ruler_header_border_top = egui::Pos2::new(header_right_x, full_track_rect.min.y);
                let ruler_header_border_bottom = egui::Pos2::new(header_right_x, full_track_rect.max.y);
                // Use grey border to match the header divider
                let header_border = egui::Stroke {
                    width: 1.0,
                    color: egui::Color32::from_rgb(128, 128, 128), // Grey
                };
                self.ui.painter().line_segment([ruler_header_border_top, ruler_header_border_bottom], header_border);
            }
        } else {
            // Regular tracks: draw complete borders around track content (all 4 sides)
            // Since we have 4px spacing between tracks, each track gets its own complete border
            let left_bottom = egui::Pos2::new(full_track_rect.min.x, full_track_rect.max.y);
            let right_bottom = egui::Pos2::new(full_track_rect.max.x, full_track_rect.max.y);
            
            // Draw borders in order: left, right, bottom, then top LAST to ensure it's on top of everything
            let left_top = egui::Pos2::new(full_track_rect.min.x, full_track_rect.min.y);
            let right_top = egui::Pos2::new(full_track_rect.max.x, full_track_rect.min.y);
            
            // Left border
            self.ui.painter().line_segment([left_top, left_bottom], pink_border);
            // Right border
            self.ui.painter().line_segment([right_top, right_bottom], pink_border);
            // Bottom border (at the bottom of the track, before spacing)
            self.ui.painter().line_segment([left_bottom, right_bottom], pink_border);
            // Top border: draw it slightly inside the rect (0.5px) to ensure it's not clipped
            let top_border_y = full_track_rect.min.y + 0.5;
            let top_left = egui::Pos2::new(full_track_rect.min.x, top_border_y);
            let top_right = egui::Pos2::new(full_track_rect.max.x, top_border_y);
            self.ui.painter().line_segment([top_left, top_right], pink_border);
            
            // Draw right border for the header area to separate it from the timeline/grid
            if let Some(header_rect) = self.tracks.header_full_rect {
                let header_right_x = header_rect.max.x;
                // Header border starts at the track's top (with offset for first track)
                let header_border_top_y = full_track_rect.min.y;
                let header_border_top = egui::Pos2::new(header_right_x, header_border_top_y);
                let header_border_bottom = egui::Pos2::new(header_right_x, full_track_rect.max.y);
                // Use grey border for the header divider to differentiate from track borders
                let header_border = egui::Stroke {
                    width: 1.0,
                    color: egui::Color32::from_rgb(128, 128, 128), // Grey
                };
                self.ui.painter().line_segment([header_border_top, header_border_bottom], header_border);
            }
        }
        
        // Manually add space occuppied by the child UIs, otherwise `ScrollArea` won't consider the
        // space occuppied. The spacing is added AFTER the border is drawn, so borders are tight around tracks.
        let w = self.tracks.full_rect.width();
        let h = full_track_height;
        // Add spacing after track (except for last track)
        // Ruler (first pinned track, index 0) uses 4px spacing, Marker (second pinned track, index 1) uses 10px spacing
        // Regular tracks ALWAYS use 4px spacing
        // This spacing ensures the next track starts at this track's bottom border + spacing
        // Pinned tracks have track_id = None (they don't call with_id()), regular tracks have track_id = Some(...)
        let spacing_after = if self.is_last_track {
            0.0
        } else if self.track_id.is_none() && self.pinned_track_index.is_some() {
            // Pinned track: Ruler (index 0) uses 4px, Marker (index 1) uses 10px
            let pinned_index = self.pinned_track_index.unwrap();
            if pinned_index == 0 {
                // Ruler (first pinned track): 4px spacing
                4.0
            } else {
                // Marker (second pinned track): 10px spacing
                10.0
            }
        } else {
            // Regular tracks: ALWAYS 4px spacing (regardless of pinned_track_index)
            4.0
        };
        // Add spacing directly to parent UI (not in scope) to ensure it's properly consumed
        // This ensures the next track's available_rect is correctly positioned
        // The space added must account for the track offset so the next track starts at:
        // current track's actual bottom (including offset) + 4px spacing
        self.ui.spacing_mut().item_spacing.y = 0.0;
        self.ui.spacing_mut().interact_size.y = 0.0;
        self.ui.horizontal(|ui| ui.add_space(w));
        // Add space equal to: track height + offset + spacing
        // This ensures next track starts at: (current track top + offset + height) + 4px = current track bottom + 4px
        self.ui.add_space(h + track_offset_y + spacing_after);
    }
}

impl TimelineCtx {
    /// The number of visible ticks across the width of the timeline.
    pub fn visible_ticks(&self) -> f32 {
        self.visible_ticks
    }

    /// Get the left edge X position where tick 0 should be displayed.
    pub fn left_edge_x(&self) -> f32 {
        self.full_rect.min.x
    }
}

// Internal access for timeline module
impl TracksCtx {
    pub(crate) fn new(full_rect: Rect, header_full_rect: Option<Rect>, timeline: TimelineCtx) -> Self {
        Self {
            full_rect,
            header_full_rect,
            timeline,
            pinned_track_index: Cell::new(0),
        }
    }
    
    /// Reset the pinned track counter (called when starting pinned tracks)
    pub(crate) fn reset_pinned_track_index(&self) {
        self.pinned_track_index.set(0);
    }
    
    /// Get and increment the pinned track index (for pinned tracks only)
    pub(crate) fn next_pinned_track_index(&self) -> usize {
        let index = self.pinned_track_index.get();
        self.pinned_track_index.set(index + 1);
        index
    }
}

impl TimelineCtx {
    pub(crate) fn new(full_rect: Rect, visible_ticks: f32) -> Self {
        Self {
            full_rect,
            visible_ticks,
        }
    }
}

impl SetPlayhead {
    pub(crate) fn new(timeline_rect: Rect, tracks_top: f32, tracks_bottom: f32) -> Self {
        Self {
            timeline_rect,
            tracks_top,
            tracks_bottom,
            bottom_bar_rect: None,
            top_panel_rect: None,
        }
    }

    pub(crate) fn timeline_rect(&self) -> Rect {
        self.timeline_rect
    }

    pub(crate) fn tracks_top(&self) -> f32 {
        self.tracks_top
    }

    pub(crate) fn tracks_bottom(&self) -> f32 {
        self.tracks_bottom
    }

}
